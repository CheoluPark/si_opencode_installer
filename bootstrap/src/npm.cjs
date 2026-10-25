const { mkdtemp, mkdir, rm, writeFile } = require('node:fs/promises');
const { spawnSync } = require('node:child_process');
const { tmpdir } = require('node:os');
const { join } = require('node:path');

async function installFromNpm(artifact) {
  const temporary = await mkdtemp(join(tmpdir(), 'si-opencode-npm-'));
  const npmrc = join(temporary, '.npmrc');
  try {
    await writeFile(npmrc, `registry=${artifact.npmRegistry}\nalways-auth=true\n`, 'utf8');
    const npm = process.env.SI_BOOTSTRAP_NPM_COMMAND || 'npm';
    const result = spawnSync(npm, [
      'install', '--prefix', temporary, '--ignore-scripts', '--no-audit', '--no-fund', '--package-lock=false',
      `${artifact.npmPackage}@${artifact.npmVersion}`, '--registry', artifact.npmRegistry,
    ], {
      encoding: 'utf8',
      shell: process.platform === 'win32' && npm.toLowerCase().endsWith('.cmd'),
      env: { ...process.env, NPM_CONFIG_USERCONFIG: npmrc },
    });
    if (result.error || result.status !== 0) {
      const detail = (result.error?.message || result.stderr || result.stdout || `exit code ${result.status}`).trim();
      throw new Error(detail);
    }
    return { root: temporary, source: join(temporary, 'node_modules', ...artifact.npmPackage.split('/'), artifact.npmPath) };
  } catch (error) {
    await rm(temporary, { recursive: true, force: true });
    throw error;
  }
}

async function cleanupNpmInstall(result) {
  if (result) await rm(result.root, { recursive: true, force: true });
}

module.exports = { cleanupNpmInstall, installFromNpm };
