const { mkdir, appendFile } = require('node:fs/promises');
const { dirname } = require('node:path');

function parseArgs(args) {
  const options = { releaseRoot: process.cwd(), prepareOnly: false };
  for (let index = 0; index < args.length; index += 1) {
    const value = args[index];
    if (value === '--release-root') options.releaseRoot = args[++index];
    else if (value === '--prepare-only') options.prepareOnly = true;
    else if (value === '--help' || value === '-h') options.help = true;
    else throw new Error(`Unknown argument: ${value}`);
  }
  return options;
}

async function log(logPath, message) {
  await mkdir(dirname(logPath), { recursive: true });
  await appendFile(logPath, `${new Date().toISOString()} ${message}\n`, 'utf8');
}

module.exports = { log, parseArgs };
