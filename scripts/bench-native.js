'use strict'

/* eslint-disable no-console */

const fs = require('node:fs/promises')
const os = require('node:os')
const path = require('node:path')
const tar = require('tar')
const cacache = require('cacache')
const ssri = require('ssri')

const native = require('../lib/native/index.js')
const patchPacote = require('../lib/native/patch-pacote.js')
const FetcherBase = require('pacote/lib/fetcher.js')
const FileFetcher = require('pacote/lib/file.js')

const originalExtract = FetcherBase.prototype.extract

const walk = async (dir, base = '') => {
  const out = []
  for (const entry of await fs.readdir(dir, { withFileTypes: true })) {
    const rel = base ? `${base}/${entry.name}` : entry.name
    const full = path.join(dir, entry.name)
    const stats = await fs.lstat(full)
    if (entry.isDirectory()) {
      out.push(`${rel}/`, ...await walk(full, rel))
    } else if (entry.isSymbolicLink()) {
      out.push(`${rel}->${await fs.readlink(full)}`)
    } else {
      out.push(`${rel}:${(stats.mode & 0o777).toString(8)}:${(await fs.readFile(full)).toString('base64')}`)
    }
  }
  return out.sort()
}

const makeTarball = async (root, files, { gzip = true } = {}) => {
  const stage = await fs.mkdtemp(path.join(root, 'stage-'))
  const pkg = path.join(stage, 'package')
  await fs.mkdir(pkg, { recursive: true })
  for (const [name, content] of Object.entries(files)) {
    const target = path.join(pkg, name)
    await fs.mkdir(path.dirname(target), { recursive: true })
    await fs.writeFile(target, content)
  }
  const tarball = path.join(stage, 'fixture.tgz')
  await tar.c({ gzip, cwd: stage, file: tarball }, ['package'])
  const data = await fs.readFile(tarball)
  return { tarball, data, integrity: String(ssri.fromData(data, { algorithms: ['sha512'] })) }
}

const median = (values) => {
  const sorted = [...values].sort((a, b) => a - b)
  const mid = Math.floor(sorted.length / 2)
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2
}

const format = (ms) => `${ms.toFixed(1)}ms`

const check = async (root) => {
  const files = {
    'package.json': JSON.stringify({ name: 'fixture', version: '1.0.0', bin: { fixture: 'bin/cli.js' } }),
    'bin/cli.js': '#!/usr/bin/env node\n',
    '.gitignore': 'node_modules\n',
    'lib/index.js': 'module.exports = 1\n',
    [`${'nested/'.repeat(12)}deep.js`]: 'deep\n',
    'папка/файл.js': 'unicode\n',
  }
  const fixture = await makeTarball(root, files)
  const cache = path.join(root, 'cache')
  await cacache.put(cache, `pacote:tarball:file:${fixture.tarball}`, fixture.data)
  const opts = { cache, integrity: fixture.integrity }
  const jsDest = path.join(root, 'check-js')
  const nativeDest = path.join(root, 'check-native')

  await originalExtract.call(new FileFetcher(`file:${fixture.tarball}`, opts), jsDest)
  const before = { ...native.stats }
  await new FileFetcher(`file:${fixture.tarball}`, opts).extract(nativeDest)
  const treeA = await walk(jsDest)
  const treeB = await walk(nativeDest)
  if (JSON.stringify(treeA) !== JSON.stringify(treeB)) {
    console.error('check: extracted trees differ')
    process.exitCode = 1
    return
  }
  if (native.stats.warm - before.warm !== 1) {
    console.error('check: warm native path was not used')
    process.exitCode = 1
    return
  }

  const plain = await makeTarball(root, { 'package.json': '{"name":"p","version":"1.0.0"}' }, { gzip: false })
  const plainCache = path.join(root, 'cache-plain')
  await cacache.put(plainCache, `pacote:tarball:file:${plain.tarball}`, plain.data)
  const plainOpts = { cache: plainCache, integrity: plain.integrity }
  const plainJs = path.join(root, 'plain-js')
  const plainNative = path.join(root, 'plain-native')
  await originalExtract.call(new FileFetcher(`file:${plain.tarball}`, plainOpts), plainJs)
  const beforePlain = { ...native.stats }
  await new FileFetcher(`file:${plain.tarball}`, plainOpts).extract(plainNative)
  if (JSON.stringify(await walk(plainJs)) !== JSON.stringify(await walk(plainNative))) {
    console.error('check: non-gzip fallback differs')
    process.exitCode = 1
    return
  }
  if (native.stats.fallback - beforePlain.fallback !== 1) {
    console.error('check: non-gzip tarball did not fall back')
    process.exitCode = 1
    return
  }

  const wrong = String(ssri.fromData(Buffer.from('nope'), { algorithms: ['sha512'] }))
  try {
    await new FileFetcher(`file:${plain.tarball}`, { cache: plainCache, integrity: wrong })
      .extract(path.join(root, 'bad'))
    console.error('check: integrity mismatch did not throw')
    process.exitCode = 1
  } catch (err) {
    if (err.code !== 'EINTEGRITY') {
      console.error(`check: expected EINTEGRITY, got ${err.code}`)
      process.exitCode = 1
    }
  }

  const removalRoot = path.join(root, 'rm')
  const targets = []
  for (let i = 0; i < 20; i++) {
    const dir = path.join(removalRoot, `pkg-${i}`)
    await fs.mkdir(path.join(dir, 'lib'), { recursive: true })
    await fs.writeFile(path.join(dir, 'lib', 'index.js'), 'x')
    targets.push(dir)
  }
  if (!await native.removeMany(targets)) {
    console.error('check: remove_many failed')
    process.exitCode = 1
    return
  }
  for (const target of targets) {
    const exists = await fs.stat(target).then(() => true, () => false)
    if (exists) {
      console.error(`check: ${target} still exists`)
      process.exitCode = 1
    }
  }

  if (!process.exitCode) {
    console.log('check: all differential checks passed')
  }
}

const benchExtract = async (root, iterations) => {
  const files = { 'package.json': JSON.stringify({ name: 'bench', version: '1.0.0' }) }
  for (let i = 0; i < 40; i++) {
    files[`lib/dir-${i % 8}/file-${i}.js`] = `module.exports = ${i}\n`.repeat(20)
  }
  const fixture = await makeTarball(root, files)
  const cache = path.join(root, 'bench-cache')
  await cacache.put(cache, `pacote:tarball:file:${fixture.tarball}`, fixture.data)
  const opts = { cache, integrity: fixture.integrity }
  const jsTimes = []
  const nativeTimes = []
  for (let i = 0; i < iterations; i++) {
    const fetcher = new FileFetcher(`file:${fixture.tarball}`, opts)
    let start = process.hrtime.bigint()
    await originalExtract.call(fetcher, path.join(root, `js-${i}`))
    jsTimes.push(Number(process.hrtime.bigint() - start) / 1e6)

    start = process.hrtime.bigint()
    await new FileFetcher(`file:${fixture.tarball}`, opts).extract(path.join(root, `native-${i}`))
    nativeTimes.push(Number(process.hrtime.bigint() - start) / 1e6)
  }
  return { js: median(jsTimes), native: median(nativeTimes), count: 40 }
}

const benchRemove = async (root, iterations, fileCount) => {
  const makeTree = async (dir) => {
    for (let i = 0; i < fileCount; i++) {
      const target = path.join(dir, `mod-${i % 250}`, `nested-${i % 10}`, `file-${i}.js`)
      await fs.mkdir(path.dirname(target), { recursive: true })
      await fs.writeFile(target, 'x')
    }
  }
  const jsTimes = []
  const nativeTimes = []
  for (let i = 0; i < iterations; i++) {
    const jsDir = path.join(root, `js-rm-${i}`)
    await makeTree(jsDir)
    const jsEntries = await fs.readdir(jsDir)
    let start = process.hrtime.bigint()
    await Promise.all(jsEntries.map(entry =>
      fs.rm(path.join(jsDir, entry), { recursive: true, force: true })))
    jsTimes.push(Number(process.hrtime.bigint() - start) / 1e6)

    const nativeDir = path.join(root, `native-rm-${i}`)
    await makeTree(nativeDir)
    const nativeEntries = await fs.readdir(nativeDir)
    start = process.hrtime.bigint()
    await native.removeMany(nativeEntries.map(entry => path.join(nativeDir, entry)))
    nativeTimes.push(Number(process.hrtime.bigint() - start) / 1e6)
  }
  return { js: median(jsTimes), native: median(nativeTimes), count: fileCount }
}

const main = async () => {
  if (!native.isAvailable()) {
    console.error(`native core is not available (expected ${require('../lib/native/paths.js').binary})`)
    process.exit(1)
  }
  patchPacote()

  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'npmr-bench-'))
  try {
    if (process.argv.includes('--check')) {
      await check(root)
      return
    }
    if (process.env.NPMPR_BENCH_FILES) {
      const count = Number(process.env.NPMPR_BENCH_FILES)
      const result = await benchRemove(root, 3, count)
      console.log(`remove (${result.count} files): js ${format(result.js)} ` +
        `native ${format(result.native)} x${(result.js / result.native).toFixed(2)}`)
      return
    }
    const extract = await benchExtract(root, 8)
    console.log(`extract (${extract.count} files): js ${format(extract.js)} ` +
      `native ${format(extract.native)} x${(extract.js / extract.native).toFixed(2)}`)
    const remove = await benchRemove(root, 3, 2500)
    console.log(`remove (${remove.count} files): js ${format(remove.js)} ` +
      `native ${format(remove.native)} x${(remove.js / remove.native).toFixed(2)}`)
  } finally {
    await fs.rm(root, { recursive: true, force: true })
  }
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
