#!/usr/bin/env node
/*
 * Postbuild step: stitches the hand-maintained JS facade and KlockHttpClient onto NAPI-RS's
 * auto-generated `index.js` and `index.d.ts`. Without this, `napi build`
 * regenerates both files from the Rust source and erases the JS-only
 * HTTP client, breaking `import { KlockHttpClient } from '@klock-protocol/core'`.
 *
 * Idempotent: detects a marker line and exits early if already applied.
 */
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '..')

const indexJs = path.join(root, 'index.js')
const indexDts = path.join(root, 'index.d.ts')
const httpJs = path.join(root, 'klock-http-client.js')
const httpDts = path.join(root, 'klock-http-client.d.ts')
const facadeDts = path.join(root, 'klock-facade.d.ts')

const jsMarker = '// === klock-http-client.js (appended by postbuild) ==='
const dtsMarker = '// === klock-http-client.d.ts (appended by postbuild) ==='
const facadeDtsMarker = '// === klock-facade.d.ts (appended by postbuild) ==='

function appendIfMissing(target, source, marker, header) {
  const current = fs.readFileSync(target, 'utf8')
  if (current.includes(marker)) {
    return false
  }
  const addition = fs.readFileSync(source, 'utf8')
  const merged = `${current.trimEnd()}\n\n${marker}\n${header}${addition}`
  fs.writeFileSync(target, merged, 'utf8')
  return true
}

// For the JS side we need to splice the class into the require()-based
// module so the existing module.exports.KlockClient stays intact and we
// add KlockHttpClient alongside it. To keep things simple, we re-require
// the file from disk: it's published in the package, runs at module load,
// and pulls KlockHttpClient into the same module.exports object.
const jsBridge =
  "// Pull the JS-only HTTP client and facade into this module's exports so users\n" +
  "// can `const { Klock, KlockHttpClient } = require('@klock-protocol/core')`.\n" +
  "module.exports.KlockHttpClient = require('./klock-http-client').KlockHttpClient\n" +
  "module.exports.Klock = require('./klock-facade').createKlockFacade(KlockClient)\n"

const currentJs = fs.readFileSync(indexJs, 'utf8')
if (!currentJs.includes(jsMarker)) {
  const merged = `${currentJs.trimEnd()}\n\n${jsMarker}\n${jsBridge}`
  fs.writeFileSync(indexJs, merged, 'utf8')
  console.log('Wired KlockHttpClient into index.js')
}

if (appendIfMissing(indexDts, httpDts, dtsMarker, '')) {
  console.log('Appended klock-http-client.d.ts onto index.d.ts')
}

if (appendIfMissing(indexDts, facadeDts, facadeDtsMarker, '')) {
  console.log('Appended klock-facade.d.ts onto index.d.ts')
}
