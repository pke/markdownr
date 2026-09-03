import {spawnSync} from 'node:child_process';
import {describe, expect, it} from 'vitest';

const parseQueryScript = `
const queryString = require('query-string');
const result = queryString.parse('route=hello%20world');
process.stdout.write(result.route);
`;

describe('query-string compatibility', () => {
  it('decodes query values through its CommonJS entry point', () => {
    const result = spawnSync(process.execPath, ['-e', parseQueryScript], {
      encoding: 'utf8',
    });

    expect(result.stderr).toBe('');
    expect(result.status).toBe(0);
    expect(result.stdout).toBe('hello world');
  });
});
