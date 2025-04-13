import { describe, expect, it } from 'vitest';
import { openRepository } from '../index';
import { useFixture } from './fixtures';

describe('blame', () => {
  const signature1 = { name: 'Seokju Na', email: 'seokju.me@gmail.com' };
  const signature2 = { name: 'Seokju Me', email: 'seokju.me@toss.im' };

  describe('getBlame', () => {
    it('should return all blame hunks for the file', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);

      const hunks = repo.getBlame('blame');
      expect(hunks.length).toBeGreaterThan(0);
      expect(hunks[0]?.commitId).toBeTruthy();
      expect(hunks[0]?.finalStartLineNumber).toBeGreaterThan(0);
      expect(hunks[0]?.linesInHunk).toBeGreaterThan(0);
    });

    it('should support blame options', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);

      const oldestCommit = 'a6c10f4d68f91e51c9a1e664e4b1efa0d2265edd';
      const hunks = repo.getBlame('blame');
      const hunks2 = repo.getBlame('blame', { oldestCommit });

      expect(hunks.length).toBeGreaterThan(hunks2.length);
    });

    it('should support trackLinesMovement option', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);

      const hunks = repo.getBlame('blame', { trackLinesMovement: true });
      expect(hunks.length).toBeGreaterThan(0);
    });

    it('should throw for non-existent file', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);

      expect(() => repo.getBlame('non-existent-file')).toThrow();
    });

    it('should return correct blame information for all lines', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);

      const hunks = repo.getBlame('blame');

      const lineHunks = new Map();

      // biome-ignore lint/complexity/noForEach: <explanation>
      hunks.forEach(hunk => {
        const start = hunk.finalStartLineNumber;
        for (let i = 0; i < hunk.linesInHunk; i++) {
          lineHunks.set(start + i, hunk);
        }
      });

      // biome-ignore lint/complexity/noForEach: <explanation>
      [1, 2, 3, 4].forEach(lineNum => {
        const hunk = lineHunks.get(lineNum);
        if (!hunk) {
          throw new Error(`Hunk for line ${lineNum} not found`);
        }

        const expectedSignature = lineNum === 1 || lineNum === 3 ? signature1 : signature2;
        expect(hunk.signature?.name).toBe(expectedSignature.name);
      });
    });

    it('should support newest commit option', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);

      const newestCommit = '2021365d09de3644ffe28c2f332a43ba129ead75';
      const hunks = repo.getBlame('blame', { newestCommit });

      const line2Hunk = hunks.find(h => h.finalStartLineNumber <= 2 && h.finalStartLineNumber + h.linesInHunk > 2);

      if (!line2Hunk) {
        throw new Error('Line 2 hunk not found');
      }
      expect(line2Hunk.signature?.name).toBe(signature1.name);

      const standardHunks = repo.getBlame('blame');
      expect(hunks.length).not.toEqual(standardHunks.length);
    });

    it('should support combined oldest/newest commit options', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);

      const newestCommit = '2021365d09de3644ffe28c2f332a43ba129ead75';
      const oldestCommit = 'a6c10f4d68f91e51c9a1e664e4b1efa0d2265edd';

      const combinedHunks = repo.getBlame('blame', {
        oldestCommit,
        newestCommit,
      });

      const standardHunks = repo.getBlame('blame');
      expect(combinedHunks.length).toBeLessThanOrEqual(standardHunks.length);
    });
  });

  describe('getBlameLine', () => {
    it('should return blame hunk for a specific line', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);

      const line1Hunk = repo.getBlameLine('blame', 1);
      expect(line1Hunk.commitId).toBeTruthy();
      expect(line1Hunk.finalStartLineNumber).toBe(1);
      expect(line1Hunk.signature?.name).toBe(signature1.name);

      const line2Hunk = repo.getBlameLine('blame', 2);
      expect(line2Hunk.signature?.name).toBe(signature2.name);

      const line3Hunk = repo.getBlameLine('blame', 3);
      expect(line3Hunk.signature?.name).toBe(signature1.name);

      const line4Hunk = repo.getBlameLine('blame', 4);
      expect(line4Hunk.signature?.name).toBe(signature2.name);
    });

    it('should throw for invalid line number', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);
      console.log(repo.getBlameLine('blame', 99999));
      expect(() => repo.getBlameLine('blame', 999)).toThrow();
    });

    it('should support options', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);

      const newestCommit = '2021365d09de3644ffe28c2f332a43ba129ead75';
      const hunk = repo.getBlameLine('blame', 2, { newestCommit });

      expect(hunk.signature?.name).toBe(signature1.name);
    });
  });

  describe('getBlameRange', () => {
    it('should return blame hunks for a range of lines', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);

      const hunks = repo.getBlameRange('blame', 1, 3);
      expect(hunks.length).toBeGreaterThan(0);

      const lineNumbers = hunks.flatMap(h => {
        const start = h.finalStartLineNumber;
        return Array.from({ length: h.linesInHunk }, (_, i) => start + i);
      });

      expect(lineNumbers).toContain(1);
      expect(lineNumbers).toContain(2);
      expect(lineNumbers).toContain(3);
      expect(lineNumbers).not.toContain(4);
    });

    it('should throw when start line is greater than end line', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);

      expect(() => repo.getBlameRange('blame', 3, 1)).toThrow();
    });

    it('should support limited range', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);

      const hunks = repo.getBlameRange('blame', 2, 2);
      expect(hunks.length).toBe(1);
      expect(hunks[0]?.finalStartLineNumber).toBe(2);
      expect(hunks[0]?.signature?.name).toBe(signature2.name);
    });

    it('should support options', async () => {
      const p = await useFixture('blame');
      const repo = await openRepository(p);

      const newestCommit = '2021365d09de3644ffe28c2f332a43ba129ead75';
      const hunks = repo.getBlameRange('blame', 2, 4, { newestCommit });
      const lineHunks = new Map();

      // biome-ignore lint/complexity/noForEach: <explanation>
      hunks.forEach(hunk => {
        const start = hunk.finalStartLineNumber;
        for (let i = 0; i < hunk.linesInHunk; i++) {
          lineHunks.set(start + i, hunk);
        }
      });

      const hunk2 = lineHunks.get(2);
      if (!hunk2) {
        throw new Error('Line 2 hunk not found');
      }
      expect(hunk2.signature?.name).toBe(signature1.name);
    });
  });
});
