const { copyFile, mkdir, rm } = require('node:fs/promises');
const { join, dirname } = require('node:path');
const { cleanupNpmInstall, installFromNpm } = require('./npm.cjs');
const { stageDirectCopy } = require('./fallback.cjs');
const { verifyArtifact } = require('./verify.cjs');

async function stageNpm(outputManaged, path, artifact) {
  let installed;
  try {
    installed = await installFromNpm(artifact);
    await verifyArtifact(installed.source, artifact);
    const destination = join(outputManaged, path);
    await mkdir(dirname(destination), { recursive: true });
    await copyFile(installed.source, destination);
    await verifyArtifact(destination, artifact);
  } finally {
    await cleanupNpmInstall(installed);
  }
}

async function prepare({ entries, offlinePayload, preparedRoot, log }) {
  await rm(preparedRoot, { recursive: true, force: true });
  const outputManaged = join(preparedRoot, 'managed');
  for (const [path, artifact] of entries) {
    try {
      if (artifact.installMethod === 'directCopy') {
        await stageDirectCopy(offlinePayload, outputManaged, path, artifact);
        await log(`directCopy ${path}`);
        continue;
      }
      try {
        await stageNpm(outputManaged, path, artifact);
        await log(`npm ${path} ${artifact.npmPackage}@${artifact.npmVersion}`);
      } catch (npmError) {
        if (artifact.fallbackToDirectCopy === true) {
          await stageDirectCopy(offlinePayload, outputManaged, path, artifact);
          await log(`npm failed; fallback directCopy ${path}: ${npmError.message}`);
        } else {
          throw new Error(`npm installation failed for ${path}: ${npmError.message}`);
        }
      }
    } catch (error) {
      if (artifact.optional) {
        await log(`optional artifact skipped ${path}: ${error.message}`);
      } else {
        throw error;
      }
    }
  }
}

module.exports = { prepare };
