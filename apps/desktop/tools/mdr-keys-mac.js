// Sends keys straight to one process (CGEventPostToPid), for testing markdownr on macOS without
// touching other apps.  osascript -l JavaScript tools/mdr-keys-mac.js <pid> <key>...
// A key is a name or letter with optional cmd+/ctrl+/shift+/alt+ prefixes ("cmd+t", "pgdn",
// "shift+return"), "type:text" for text, "wheel:-5" for wheel lines, or "sleep:ms".
ObjC.import("CoreGraphics");
ObjC.import("Foundation");
const CODES = { a: 0, s: 1, d: 2, f: 3, h: 4, g: 5, z: 6, x: 7, c: 8, v: 9, b: 11, q: 12, w: 13, e: 14, r: 15, y: 16, t: 17,
  "1": 18, "2": 19, "3": 20, "4": 21, "6": 22, "5": 23, "=": 24, "9": 25, "7": 26, "-": 27, "8": 28, "0": 29, o: 31, u: 32,
  i: 34, p: 35, return: 36, l: 37, j: 38, k: 40, n: 45, m: 46, tab: 48, space: 49, backspace: 51, esc: 53,
  plus: 30, minus: 44, // German layout (the test Mac)
  f5: 96, f3: 99, f11: 103, home: 115, pgup: 116, f1: 122, end: 119, pgdn: 121, left: 123, right: 124, down: 125, up: 126 };
const FLAGS = { shift: 1 << 17, ctrl: 1 << 18, alt: 1 << 19, cmd: 1 << 20 };
function run(argv) {
  const pid = parseInt(argv[0]);
  for (const spec of argv.slice(1)) {
    if (spec.startsWith("sleep:")) { $.NSThread.sleepForTimeInterval(parseInt(spec.slice(6)) / 1000); continue; }
    if (spec.startsWith("click:")) {
      // x,y in the window (points from its top left); the window found by the pid.
      const [x, y] = spec.slice(6).split(",").map(Number);
      const l = ObjC.castRefToObject($.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly, 0));
      let b = null, wid = 0;
      for (let i = 0; i < l.count; i++) { const w = ObjC.deepUnwrap(l.objectAtIndex(i)); if (w.kCGWindowOwnerPID === pid && w.kCGWindowLayer === 0) { b = w.kCGWindowBounds; wid = w.kCGWindowNumber; } }
      if (!b) throw new Error("no window");
      const p = $.CGPointMake(b.X + x, b.Y + y);
      for (const type of [1, 2]) { // left mouse down, up
        const e = $.CGEventCreateMouseEvent(null, type, p, 0);
        $.CGEventSetIntegerValueField(e, 1, 1); // click state
        $.CGEventSetIntegerValueField(e, 91, wid); // window under the pointer
        $.CGEventSetIntegerValueField(e, 92, wid); // … that can handle the event
        $.CGEventPostToPid(pid, e);
        $.NSThread.sleepForTimeInterval(0.05);
      }
      continue;
    }
    if (spec.startsWith("wheel:")) {
      const e = $.CGEventCreateScrollWheelEvent(null, 1, 1, parseInt(spec.slice(6)));
      $.CGEventPostToPid(pid, e); continue;
    }
    if (spec.startsWith("type:")) {
      // Letters, digits and spaces by key code (a US layout; shift for capitals).
      for (const ch of spec.slice(5)) {
        const code = CODES[ch === " " ? "space" : ch.toLowerCase()];
        if (code === undefined) throw new Error("cannot type " + ch);
        for (const down of [true, false]) {
          const e = $.CGEventCreateKeyboardEvent(null, code, down);
          if (ch !== ch.toLowerCase()) $.CGEventSetFlags(e, FLAGS.shift);
          $.CGEventPostToPid(pid, e);
        }
      }
      continue;
    }
    const parts = spec.split("+").filter((p, i, a) => p !== "" || i === a.length - 1);
    let key = parts.pop(); if (key === "") key = "=";
    let flags = 0; for (const p of parts) flags |= FLAGS[p];
    const code = CODES[key.toLowerCase()];
    if (code === undefined) throw new Error("unknown key " + spec);
    for (const down of [true, false]) {
      const e = $.CGEventCreateKeyboardEvent(null, code, down);
      $.CGEventSetFlags(e, flags);
      $.CGEventPostToPid(pid, e);
    }
    $.NSThread.sleepForTimeInterval(0.15);
  }
  return "ok";
}
