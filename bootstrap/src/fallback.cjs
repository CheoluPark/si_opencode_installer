const { copyFile, mkdir } = require('node:fs/promises');
const { join, dirname } = require('node:path');
const { verifyArtifact } = require('./verify.cjs');

async function stageDirectCopy(offlinePayload, outputManaged, relativePath, artifact) {
  const source = join(offlinePayload, artifact.directPath);
  await verifyArtifact(source, artifact);
  const destination = join(outputManaged, relativePath);
  await mkdir(dirname(destination), { recursive: true });
  await copyFile(source, destination);
  await verifyArtifact(destination, artifact);
}

module.exports = { stageDirectCopy };
