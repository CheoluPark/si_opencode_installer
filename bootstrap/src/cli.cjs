const { existsSync } = require('node:fs');
const { cp } = require('node:fs/promises');
const { spawnSync } = require('node:child_process');
const { join, resolve } = require('node:path');
const { loadManifest } = require('./manifest.cjs');
const { prepare } = require('./orchestrator.cjs');
const { log, parseArgs } = require('./utils.cjs');

function usage() {
  return 'Usage: si-opencode-bootstrap [--release-root <folder>] [--prepare-only]';
}

async function main(args) {
  try {
    const options = parseArgs(args);
    if (options.help) {
      console.log(usage());
      return;
    }
    const root = resolve(options.releaseRoot);
    const offlinePayload = join(root, 'offline-payload');
    const logPath = join(root, 'si-opencode-bootstrap.log');
    await log(logPath, `bootstrap started releaseRoot=${root}`);
    const entries = await loadManifest(offlinePayload);
    await prepare({ entries, offlinePayload, preparedRoot: join(root, 'prepared-payload'), log: (message) => log(logPath, message) });
    await log(logPath, 'payload preparation complete');
    if (options.prepareOnly) return;
    const installer = join(root, 'si_opencode_installer.exe');
    if (!existsSync(installer)) throw new Error(`installer not found: ${installer}`);
    const installerResult = spawnSync(installer, [], { cwd: root, stdio: 'inherit' });
    if (installerResult.error || installerResult.status !== 0) {
      throw new Error(`installer failed: ${(installerResult.error?.message || `exit code ${installerResult.status}`).trim()}`);
    }
    const appDir = join(process.env.LOCALAPPDATA || '', 'Programs', 'SI OpenCode');
    const updater = join(appDir, 'si_opencode_updater.exe');
    if (!process.env.LOCALAPPDATA || !existsSync(updater)) throw new Error('installed SI OpenCode updater was not found');
    await cp(join(root, 'prepared-payload', 'managed'), join(appDir, 'managed'), { recursive: true, force: true });
    const updateResult = spawnSync(updater, ['--install-managed', appDir], { cwd: appDir, stdio: 'inherit' });
    if (updateResult.error || updateResult.status !== 0) {
      throw new Error(`prepared managed payload installation failed: ${(updateResult.error?.message || `exit code ${updateResult.status}`).trim()}`);
    }
    await log(logPath, 'installer and prepared managed payload installation complete');
  } catch (error) {
    console.error(`si-opencode-bootstrap: ${error.message}`);
    process.exitCode = 1;
  }
}

module.exports = { main };
