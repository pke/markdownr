import {spawnSync} from 'node:child_process';
import {describe, expect, it} from 'vitest';

const parseImageScript = `
const imageSize = require('image-size');
const input = Buffer.from(process.argv[1], 'base64');

try {
  imageSize(input);
} catch {}
`;

function parseInChildProcess(input: Buffer) {
  return spawnSync(
    process.execPath,
    ['-e', parseImageScript, input.toString('base64')],
    {
      encoding: 'utf8',
      timeout: 1_000,
    },
  );
}

function zeroLengthJxlBox(): Buffer {
  const input = Buffer.alloc(40);

  input.writeUInt32BE(12, 0);
  input.write('JXL ', 4, 'ascii');
  input.writeUInt32BE(20, 12);
  input.write('ftyp', 16, 'ascii');
  input.write('jxl ', 20, 'ascii');
  input.writeUInt32BE(0, 32);
  input.write('jxlp', 36, 'ascii');

  return input;
}

function zeroLengthIcnsEntry(): Buffer {
  const input = Buffer.alloc(16);

  input.write('icns', 0, 'ascii');
  input.writeUInt32BE(input.length, 4);
  input.write('icp4', 8, 'ascii');
  input.writeUInt32BE(0, 12);

  return input;
}

describe('image-size denial-of-service regressions', () => {
  it('terminates when a JXL partial-stream box has a zero-valued size', () => {
    const result = parseInChildProcess(zeroLengthJxlBox());

    expect(result.error).toBeUndefined();
    expect(result.signal).toBeNull();
    expect(result.status).toBe(0);
  });

  it('terminates when an ICNS entry has a zero-valued length', () => {
    const result = parseInChildProcess(zeroLengthIcnsEntry());

    expect(result.error).toBeUndefined();
    expect(result.signal).toBeNull();
    expect(result.status).toBe(0);
  });
});
