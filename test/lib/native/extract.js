'use strict'

// The native core is explicitly enabled here; the rest of the test suite runs
// with NPM_NATIVE_CORE=0 so that it exercises the reference JS implementation.
process.env.NPM_NATIVE_CORE = '1'

const t = require('tap')
const fs = require('node:fs/promises')
const os = require('node:os')
const path = require('node:path')
const tar = require('tar')
const cacache = require('cacache')
const ssri = require('ssri')

const native = require('../../../lib/native/index.js')
const patchPacote = require('../../../lib/native/patch-pacote.js')
const FileFetcher = require('pacote/lib/file.js')
const FetcherBase = require('pacote/lib/fetcher.js')

const available = native.isAvailable()
const posix = process.platform !== 'win32'

const originalExtract = FetcherBase.prototype.extract
let patched = false

let counter = 0

const makeTarball = async (root, files, { gzip = true } = {}) => {
  const stage = path.join(root, `stage-${counter++}`)
  const pkg = path.join(stage, 'package')
  await fs.mkdir(pkg, { recursive: true })
  for (const [name, content] of Object.entries(files)) {
    const target = path.join(pkg, name)
    await fs.mkdir(path.dirname(target), { recursive: true })
    if (content && typeof content === 'object' && content.symlink) {
      await fs.symlink(content.symlink, target)
    } else {
      await fs.writeFile(target, content)
    }
  }
  const tarball = path.join(root, `fixture-${counter++}.tgz`)
  await tar.c({ gzip, cwd: stage, file: tarball }, ['package'])
  const data = await fs.readFile(tarball)
  return { tarball, data, integrity: String(ssri.fromData(data, { algorithms: ['sha512'] })) }
}

const walk = async (dir, base = '') => {
  const entries = await fs.readdir(dir, { withFileTypes: true })
  const out = []
  for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
    const rel = base ? `${base}/${entry.name}` : entry.name
    const full = path.join(dir, entry.name)
    const stats = await fs.lstat(full)
    if (entry.isDirectory()) {
      out.push({ rel, type: 'dir', mode: stats.mode & 0o777 })
      out.push(...await walk(full, rel))
    } else if (entry.isSymbolicLink()) {
      out.push({ rel, type: 'link', target: await fs.readlink(full) })
    } else {
      out.push({ rel, type: 'file', mode: stats.mode & 0o777, data: await fs.readFile(full) })
    }
  }
  return out
}

const compare = async (t, left, right) => {
  const a = await walk(left)
  const b = await walk(right)
  if (!posix) {
    for (const entry of [...a, ...b]) {
      delete entry.mode
    }
  }
  t.same(b, a, 'trees are identical')
}

const runBoth = async (root, fixture, opts = {}) => {
  const cache = path.join(root, `cache-${counter++}`)
  await cacache.put(cache, `pacote:tarball:file:${fixture.tarball}`, fixture.data)
  const fetcherOpts = { cache, integrity: fixture.integrity, ...opts }
  const fromA = path.join(root, `orig-${counter++}`)
  const fromB = path.join(root, `native-${counter++}`)
  const resultA = await originalExtract.call(new FileFetcher(`file:${fixture.tarball}`, fetcherOpts), fromA)
  const resultB = await new FileFetcher(`file:${fixture.tarball}`, fetcherOpts).extract(fromB)
  return { resultA, resultB, fromA, fromB }
}

if (available && !patched) {
  patched = patchPacote()
}

t.test('native core extraction', { skip: native.isAvailable() ? false : 'native core binary is not built' }, async t => {
  t.beforeEach(() => {
    for (const key of Object.keys(native.stats)) {
      native.stats[key] = 0
    }
  })

  t.test('extracts files, bins and renames .gitignore', async t => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), 'npmr-extract-'))
    t.teardown(() => fs.rm(root, { recursive: true, force: true }))
    const fixture = await makeTarball(root, {
      'package.json': JSON.stringify({ name: 'fixture', version: '1.0.0', bin: { fixture: 'bin/cli.js' } }),
      'bin/cli.js': '#!/usr/bin/env node\n',
      '.gitignore': 'node_modules\n',
      'lib/index.js': 'module.exports = 1\n',
    })
    const { resultA, resultB, fromA, fromB } = await runBoth(root, fixture)
    t.same(resultB, resultA)
    await compare(t, fromA, fromB)
    t.equal(native.stats.warm, 1, 'served from the native warm path')
    t.equal(native.stats.fallback, 0)
    t.equal(await fs.readFile(path.join(fromB, '.npmignore'), 'utf8'), 'node_modules\n')
    await t.rejects(fs.stat(path.join(fromB, '.gitignore')))
  })

  t.test('long PAX paths, unicode names and both ignore files', async t => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), 'npmr-extract-'))
    t.teardown(() => fs.rm(root, { recursive: true, force: true }))
    const longName = `${'nested/'.repeat(20)}deep.js`
    const fixture = await makeTarball(root, {
      'package.json': JSON.stringify({ name: 'fixture', version: '1.0.0' }),
      [longName]: 'deep\n',
      'папка/файл.js': 'unicode\n',
      '.gitignore': 'a\n',
      '.npmignore': 'b\n',
    })
    const { resultA, resultB, fromA, fromB } = await runBoth(root, fixture)
    t.same(resultB, resultA)
    await compare(t, fromA, fromB)
    t.equal(native.stats.warm, 1)
    t.equal(await fs.readFile(path.join(fromB, '.npmignore'), 'utf8'), 'b\n')
  })

  t.test('symlinks are skipped like npm does', async t => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), 'npmr-extract-'))
    t.teardown(() => fs.rm(root, { recursive: true, force: true }))
    let fixture
    try {
      fixture = await makeTarball(root, {
        'package.json': JSON.stringify({ name: 'fixture', version: '1.0.0' }),
        'real.js': 'real\n',
        'linked.js': { symlink: 'real.js' },
      })
    } catch (err) {
      t.skip(`cannot create symlinks: ${err.code}`)
      return
    }
    const { resultA, resultB, fromA, fromB } = await runBoth(root, fixture)
    t.same(resultB, resultA)
    await compare(t, fromA, fromB)
    t.equal(native.stats.warm, 1)
    await t.rejects(fs.lstat(path.join(fromB, 'linked.js')))
  })

  t.test('non-gzip tarball falls back to JS', async t => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), 'npmr-extract-'))
    t.teardown(() => fs.rm(root, { recursive: true, force: true }))
    const fixture = await makeTarball(root, {
      'package.json': JSON.stringify({ name: 'fixture', version: '1.0.0' }),
      'a.js': 'a\n',
    }, { gzip: false })
    const { resultA, resultB, fromA, fromB } = await runBoth(root, fixture)
    t.same(resultB, resultA)
    await compare(t, fromA, fromB)
    t.equal(native.stats.warm, 0)
    t.equal(native.stats.fallback, 1, 'JS fallback was used')
  })

  t.test('integrity mismatch falls back and surfaces EINTEGRITY', async t => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), 'npmr-extract-'))
    t.teardown(() => fs.rm(root, { recursive: true, force: true }))
    const fixture = await makeTarball(root, {
      'package.json': JSON.stringify({ name: 'fixture', version: '1.0.0' }),
    })
    const cache = path.join(root, 'cache')
    const wrong = String(ssri.fromData(Buffer.from('nope'), { algorithms: ['sha512'] }))
    const fetcher = new FileFetcher(`file:${fixture.tarball}`, { cache, integrity: wrong })
    await t.rejects(fetcher.extract(path.join(root, 'out')), { code: 'EINTEGRITY' })
    t.equal(native.stats.warm, 0)
    t.equal(native.stats.fallback, 1)
  })

  t.test('remove_many deletes trees and tolerates missing paths', async t => {
    const root = await fs.mkdtemp(path.join(os.tmpdir(), 'npmr-rm-'))
    t.teardown(() => fs.rm(root, { recursive: true, force: true }))
    const paths = []
    for (let i = 0; i < 5; i++) {
      const dir = path.join(root, `mod-${i}`)
      await fs.mkdir(path.join(dir, 'nested'), { recursive: true })
      await fs.writeFile(path.join(dir, 'nested', 'file.js'), 'x')
      paths.push(dir)
    }
    paths.push(path.join(root, 'does-not-exist'))
    t.equal(await native.removeMany(paths), true)
    for (const dir of paths) {
      await t.rejects(fs.stat(dir))
    }
    t.equal(native.stats.removal, paths.length)
  })
})
