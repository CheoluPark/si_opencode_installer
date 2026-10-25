const { readFile } = require('node:fs/promises');
const { isAbsolute, normalize, sep } = require('node:path');
const { verifyManifest } = require('./verify.cjs');

function safeRelativePath(value, field) {
  if (typeof value !== 'string' || !value || value.includes(':') || value.includes('\0') || isAbsolute(value) || normalize(value).split(sep).includes('..')) {
    throw new Error(`${field} must be a safe relative path`);
  }
  return value.replaceAll('\\', '/');
}

function normalizeArtifact(path, artifact, defaultRegistry) {
  safeRelativePath(path, 'manifest path');
  if (!artifact || typeof artifact !== 'object' || typeof artifact.sha256 !== 'string' || !Number.isSafeInteger(artifact.bytes)) {
    throw new Error(`invalid artifact metadata for ${path}`);
  }
  const installMethod = artifact.installMethod || 'directCopy';
  if (!['directCopy', 'npm'].includes(installMethod)) throw new Error(`invalid installMethod for ${path}`);
  const normalized = {
    ...artifact,
    installMethod,
    directPath: safeRelativePath(artifact.directPath || path, `directPath for ${path}`),
    optional: artifact.optional === true,
  };
  if (installMethod === 'npm') {
    if (!artifact.npmRegistry && defaultRegistry) normalized.npmRegistry = defaultRegistry;
    for (const field of ['npmPackage', 'npmVersion', 'npmPath']) {
      if (typeof artifact[field] !== 'string' || !artifact[field]) throw new Error(`${field} is required for npm artifact ${path}`);
    }
    safeRelativePath(artifact.npmPath, `npmPath for ${path}`);
  }
  return normalized;
}

async function loadManifest(offlinePayload) {
  const manifestPath = `${offlinePayload}/manifest.json`;
  const [manifestBytes, signature, publicKey] = await Promise.all([
    readFile(manifestPath),
    readFile(`${offlinePayload}/manifest.sig`, 'utf8'),
    readFile(`${offlinePayload}/publisher-ed25519.pub`),
  ]);
  verifyManifest(manifestBytes, signature, publicKey);
  const manifest = JSON.parse(manifestBytes.toString('utf8'));
  if (!manifest || !Number.isInteger(manifest.schema) || !manifest.files || typeof manifest.files !== 'object') {
    throw new Error('invalid manifest');
  }
  const defaultRegistry = manifest.delivery?.npmRegistry;
  return Object.entries(manifest.files).map(([path, artifact]) => [path, normalizeArtifact(path, artifact, defaultRegistry)]);
}

module.exports = { loadManifest };
