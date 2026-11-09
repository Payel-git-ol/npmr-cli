'use strict'

/* eslint-disable no-console */

const { spawnSync } = require('node:child_process')
const fs = require('node:fs')
const path = require('node:path')

const root = path.resolve(__dirname, '..')
const nativeDir = path.join(root, 'lib', 'native')
const debug = process.argv.includes('--debug')
const profile = debug ? 'debug' : 'release'

const args = ['build', '--manifest-path', path.join(nativeDir, 'Cargo.toml')]
if (!debug) {
  args.push('--release')
}

const result = spawnSync('cargo', args, { cwd: root, stdio: 'inherit' })
if (result.error) {
  console.error(`build-native: failed to run cargo: ${result.error.message}`)
  process.exit(1)
}
if (result.status !== 0) {
  process.exit(result.status ?? 1)
}

const binaryName = process.platform === 'win32' ? 'npmr-core.exe' : 'npmr-core'
const source = path.join(nativeDir, 'target', profile, binaryName)
const destDir = path.join(nativeDir, 'bin', `${process.platform}-${process.arch}`)
fs.mkdirSync(destDir, { recursive: true })
fs.copyFileSync(source, path.join(destDir, binaryName))
console.log(`build-native: installed ${path.relative(root, path.join(destDir, binaryName))}`)
