const { createHash, createPublicKey, verify } = require('node:crypto');
const { readFile, stat } = require('node:fs/promises');

const ED25519_SPKI_PREFIX = Buffer.from('302a300506032b6570032100', 'hex');

async function hashFile(path) {
  return createHash('sha256').update(await readFile(path)).digest('hex');
}

async function verifyArtifact(path, artifact) {
  const metadata = await stat(path);
  if (metadata.size !== artifact.bytes) throw new Error(`size mismatch for ${path}`);
  if ((await hashFile(path)) !== artifact.sha256) throw new Error(`sha256 mismatch for ${path}`);
}

function verifyManifest(manifestBytes, signatureText, rawPublicKey) {
  if (rawPublicKey.length !== 32) throw new Error('publisher-ed25519.pub must contain a 32-byte Ed25519 public key');
  const key = createPublicKey({ key: Buffer.concat([ED25519_SPKI_PREFIX, rawPublicKey]), format: 'der', type: 'spki' });
  if (!verify(null, manifestBytes, key, Buffer.from(signatureText.trim(), 'base64'))) {
    throw new Error('manifest signature verification failed');
  }
}

module.exports = { hashFile, verifyArtifact, verifyManifest };
