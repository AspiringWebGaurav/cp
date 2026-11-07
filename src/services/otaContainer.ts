/**
 * CP — OTA Encrypted Container Protocol (1:1 Downex Pattern)
 * 
 * Container Structure:
 * - 8 bytes:  Magic "CP_OTA\0\0" (or "DWNX_OTA")
 * - 16 bytes: Target Version (UTF-8 null-padded)
 * - 8 bytes:  Original Payload Size (Big-Endian uint64)
 * - 12 bytes: AES-256-GCM IV (Nonce)
 * - 16 bytes: AES-256-GCM Auth Tag
 * - N bytes:  Ciphertext
 */

const CP_OTA_SEED_SALT = 'com.cp.engine.ota.payload.v1';
const DOWNEX_OTA_SEED_SALT = 'com.downex.manager.ota.payload.v1';

async function deriveOtaKey(seedSalt: string, prefix: string): Promise<CryptoKey> {
  const enc = new TextEncoder();
  const rawKeyMaterial = enc.encode(`${prefix}-internal-ota-key-${seedSalt}`);
  const hash = await crypto.subtle.digest('SHA-256', rawKeyMaterial);
  return await crypto.subtle.importKey(
    'raw',
    hash,
    { name: 'AES-GCM', length: 256 },
    false,
    ['decrypt']
  );
}

export interface UnpackedContainer {
  version: string;
  payload: Uint8Array;
}

export async function unpackOtaPayload(rawBuffer: ArrayBuffer): Promise<UnpackedContainer> {
  const bytes = new Uint8Array(rawBuffer);

  // 1. Direct Windows PE executable check (MZ header fallback)
  if (bytes.length >= 2 && bytes[0] === 0x4D && bytes[1] === 0x5A) {
    return {
      version: 'raw',
      payload: bytes,
    };
  }

  // 2. Validate Container Size
  if (bytes.length < 60) {
    throw new Error('Corrupt update container: file size is too small');
  }

  // 3. Validate Container Magic ("CP_OTA" or "DWNX_OTA")
  const magicRaw = new TextDecoder().decode(bytes.slice(0, 8));
  const isCpOta = magicRaw.startsWith('CP_OTA');
  const isDwnxOta = magicRaw.startsWith('DWNX_OTA');

  if (!isCpOta && !isDwnxOta) {
    throw new Error(`Invalid update container: unrecognized header magic "${magicRaw.slice(0, 8)}"`);
  }

  // 4. Extract metadata
  const versionRaw = new TextDecoder().decode(bytes.slice(8, 24));
  const version = versionRaw.replace(/\0+$/, '').trim();

  // 5. Extract IV & Tag
  const iv = bytes.slice(32, 44);
  const tag = bytes.slice(44, 60);
  const ciphertext = bytes.slice(60);

  // Web Crypto AES-GCM expects ciphertext concatenated with the 16-byte auth tag at the end
  const combinedCiphertext = new Uint8Array(ciphertext.length + tag.length);
  combinedCiphertext.set(ciphertext, 0);
  combinedCiphertext.set(tag, ciphertext.length);

  // 6. Decrypt in memory (try CP key, then Downex fallback)
  const candidateKeys = isCpOta
    ? [
        await deriveOtaKey(CP_OTA_SEED_SALT, 'cp'),
        await deriveOtaKey(DOWNEX_OTA_SEED_SALT, 'downex'),
      ]
    : [
        await deriveOtaKey(DOWNEX_OTA_SEED_SALT, 'downex'),
        await deriveOtaKey(CP_OTA_SEED_SALT, 'cp'),
      ];

  let decryptedBytes: Uint8Array | null = null;
  let lastErr: any = null;

  for (const key of candidateKeys) {
    try {
      const decryptedBuffer = await crypto.subtle.decrypt(
        {
          name: 'AES-GCM',
          iv,
          tagLength: 128,
        },
        key,
        combinedCiphertext
      );
      decryptedBytes = new Uint8Array(decryptedBuffer);
      break;
    } catch (err) {
      lastErr = err;
    }
  }

  if (!decryptedBytes) {
    throw new Error(`Payload decryption / authentication failed: ${lastErr?.message || String(lastErr)}`);
  }

  // Verify PE MZ signature on decrypted executable
  if (decryptedBytes.length < 2 || decryptedBytes[0] !== 0x4D || decryptedBytes[1] !== 0x5A) {
    throw new Error('Decryption succeeded but payload is not a valid Windows executable (MZ header missing)');
  }

  return {
    version,
    payload: decryptedBytes,
  };
}

