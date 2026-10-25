const assert = require('node:assert/strict');
const { createHash, generateKeyPairSync, sign } = require('node:crypto');
const { mkdtemp, mkdir, readFile, writeFile } = require('node:fs/promises');
const { spawnSync } = require('node:child_process');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const test = require('node:test');

const cli = join(__dirname, '..', 'bin', 'si-opencode-bootstrap.cjs');

async function fixture(entries, delivery = {}) {
  const root = await mkdtemp(join(tmpdir(), 'si-bootstrap-'));
  const offline = join(root, 'offline-payload');
  await mkdir(offline, { recursive: true });
  const files = {};
  for (const entry of entries) {
    const path = join(offline, entry.directPath || entry.path);
    await mkdir(join(path, '..'), { recursive: true });
    if (entry.contents !== undefined) await writeFile(path, entry.contents);
    const bytes = entry.expectedBytes ?? Buffer.byteLength(entry.contents || '');
    files[entry.path] = {
      sha256: entry.expectedHash || createHash('sha256').update(entry.contents || '').digest('hex'),
      bytes,
      fpga: false,
      ...entry.delivery,
    };
  }
  const manifest = Buffer.from(JSON.stringify({ schema: 1, version: 1, files, settings: {}, delivery }));
  const keys = generateKeyPairSync('ed25519');
  const rawPublic = keys.publicKey.export({ format: 'der', type: 'spki' }).subarray(-32);
  await writeFile(join(offline, 'manifest.json'), manifest);
  await writeFile(join(offline, 'manifest.sig'), sign(null, manifest, keys.privateKey).toString('base64'));
  await writeFile(join(offline, 'publisher-ed25519.pub'), rawPublic);
  return { root, offline };
}

async function fakeNpm(root) {
  const bin = join(root, 'fake-npm-bin');
  const script = join(bin, 'fake-npm.cjs');
  await mkdir(bin, { recursive: true });
  await writeFile(script, `
    const { mkdirSync, writeFileSync } = require('node:fs');
    const { join } = require('node:path');
    const prefix = process.argv[process.argv.indexOf('--prefix') + 1];
    const output = join(prefix, 'node_modules', '@satreci', 'example', 'dist');
    mkdirSync(output, { recursive: true });
    writeFileSync(join(output, 'index.js'), process.env.SI_BOOTSTRAP_TEST_NPM_CONTENT);
  `);
  await writeFile(join(bin, 'npm.cmd'), `@echo off\r\n"${process.execPath}" "${script}" %*\r\n`);
  return bin;
}

function run(root, extra = {}) {
  return spawnSync(process.execPath, [cli, '--release-root', root, '--prepare-only'], {
    encoding: 'utf8',
    env: { ...process.env, ...extra },
  });
}

test('stages verified direct-copy content into the prepared managed payload', async () => {
  const { root } = await fixture([{ path: 'plugins/example/index.js', contents: 'module.exports = 1;' }]);
  const result = run(root);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(await readFile(join(root, 'prepared-payload', 'managed', 'plugins/example/index.js'), 'utf8'), 'module.exports = 1;');
});

test('rejects tampered direct-copy content before launch', async () => {
  const { root, offline } = await fixture([{ path: 'plugins/example/index.js', contents: 'known-good' }]);
  await writeFile(join(offline, 'plugins/example/index.js'), 'tampered!!');
  const result = run(root);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /sha256 mismatch/i);
});

test('uses direct-copy fallback when npm installation fails', async () => {
  const { root } = await fixture([{
    path: 'plugins/example/index.js',
    contents: 'fallback-content',
    delivery: {
      installMethod: 'npm',
      npmPackage: '@satreci/example',
      npmVersion: '1.0.0',
      npmRegistry: 'https://nexus.invalid/repository/npm-group/',
      npmPath: 'dist/index.js',
      fallbackToDirectCopy: true,
    },
  }]);
  const result = run(root, { SI_BOOTSTRAP_NPM_COMMAND: 'definitely-missing-npm-command' });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(await readFile(join(root, 'prepared-payload', 'managed', 'plugins/example/index.js'), 'utf8'), 'fallback-content');
  assert.match(await readFile(join(root, 'si-opencode-bootstrap.log'), 'utf8'), /fallback directCopy/i);
});

test('uses the signed manifest Nexus registry when an npm entry has no override', async () => {
  const { root } = await fixture([{
    path: 'plugins/example/index.js',
    contents: 'fallback-content',
    delivery: {
      installMethod: 'npm',
      npmPackage: '@satreci/example',
      npmVersion: '1.0.0',
      npmPath: 'dist/index.js',
      fallbackToDirectCopy: true,
    },
  }], { npmRegistry: 'https://nexus.example/repository/npm-group/' });
  const result = run(root, { SI_BOOTSTRAP_NPM_COMMAND: 'definitely-missing-npm-command' });
  assert.equal(result.status, 0, result.stderr);
  assert.match(await readFile(join(root, 'si-opencode-bootstrap.log'), 'utf8'), /fallback directCopy/i);
});

test('stages a version-pinned npm artifact when npm succeeds', async () => {
  const { root } = await fixture([{
    path: 'plugins/example/index.js',
    contents: 'npm-content',
    delivery: {
      installMethod: 'npm',
      npmPackage: '@satreci/example',
      npmVersion: '1.0.0',
      npmRegistry: 'https://nexus.example/repository/npm-group/',
      npmPath: 'dist/index.js',
      fallbackToDirectCopy: true,
    },
  }]);
  const bin = await fakeNpm(root);
  const result = run(root, {
    SI_BOOTSTRAP_NPM_COMMAND: join(bin, 'npm.cmd'),
    SI_BOOTSTRAP_TEST_NPM_CONTENT: 'npm-content',
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(await readFile(join(root, 'prepared-payload', 'managed', 'plugins/example/index.js'), 'utf8'), 'npm-content');
  assert.match(await readFile(join(root, 'si-opencode-bootstrap.log'), 'utf8'), /npm plugins\/example\/index\.js @satreci\/example@1\.0\.0/);
});

test('fails required npm delivery when the registry is unavailable and no fallback is supplied', async () => {
  const { root } = await fixture([{
    path: 'plugins/example/index.js',
    contents: 'not-used',
    delivery: {
      installMethod: 'npm',
      npmPackage: '@satreci/example',
      npmVersion: '1.0.0',
      npmRegistry: 'https://nexus.invalid/repository/npm-group/',
      npmPath: 'dist/index.js',
      fallbackToDirectCopy: false,
    },
  }]);
  const result = run(root, { SI_BOOTSTRAP_NPM_COMMAND: 'definitely-missing-npm-command' });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /npm installation failed/i);
});
