# Splits her arms off the resting poses so the app can swing them. Run after key.ps1.
# Arms = where the keyed master differs from the keyed body.png (art/gen/body.png is a Gemini edit of the
# master with the arms removed): the biggest differing shape on each side, holes filled, gaps closed. Each arm is cut from the
# master itself (so it matches exactly) into arm_l.png / arm_r.png; in master, talk and blink the arm areas
# are replaced with the armless body, so a swinging arm uncovers hair and torso instead of a hole.
# Shoulder pivots go to arms.json as fractions of the canvas.
param([string]$Sprites = "$PSScriptRoot\..\ui\sprites")
$ErrorActionPreference = "Stop"
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System; using System.Collections.Generic; using System.Drawing; using System.Drawing.Imaging;
using System.Runtime.InteropServices;
public static class Arms {
  static int[] Load(string path, out int w, out int h) {
    var b = new Bitmap(path); w = b.Width; h = b.Height; var px = new int[w * h];
    var d = b.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
    Marshal.Copy(d.Scan0, px, 0, px.Length); b.UnlockBits(d); b.Dispose(); return px;
  }
  static void Save(int[] px, int w, int h, string path) {
    var o = new Bitmap(w, h, PixelFormat.Format32bppArgb);
    var d = o.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
    Marshal.Copy(px, 0, d.Scan0, px.Length); o.UnlockBits(d); o.Save(path, ImageFormat.Png); o.Dispose();
  }
  static int A(int c) { return (c >> 24) & 255; }
  static int R(int c) { return (c >> 16) & 255; } static int G(int c) { return (c >> 8) & 255; } static int B(int c) { return c & 255; }
  static bool Skin(int c) { return R(c) > 180 && R(c) - B(c) > 30 && G(c) > 110; }
  static bool Dark(int c) { return Math.Max(R(c), Math.Max(G(c), B(c))) < 110 && !Hair(c); } // glove, outline
  static bool Hair(int c) { return B(c) > R(c) + 15 && B(c) > G(c) + 25; }
  static int Dist(int a, int b) {
    int dr = ((a >> 16) & 255) - ((b >> 16) & 255), dg = ((a >> 8) & 255) - ((b >> 8) & 255), db = (a & 255) - (b & 255);
    return (int)Math.Sqrt(dr * dr + dg * dg + db * db);
  }
  static bool[] Grow(bool[] m, int w, int h, int r, bool grow) { // dilate (grow) or erode by r px, square kernel
    var o = new bool[m.Length];
    for (int y = 0; y < h; y++) for (int x = 0; x < w; x++) {
      bool any = !grow;
      for (int dy = -r; dy <= r && (grow ? !any : any); dy++) for (int dx = -r; dx <= r; dx++) {
        int xx = x + dx, yy = y + dy; bool v = xx >= 0 && yy >= 0 && xx < w && yy < h && m[yy * w + xx];
        if (grow && v) { any = true; break; } if (!grow && !v) { any = false; break; } }
      o[y * w + x] = any;
    }
    return o;
  }

  public static string Run(string dir, string bodyPath) {
    int w, h; var master = Load(dir + "\\master.png", out w, out h); var body = Load(bodyPath, out w, out h);
    var diff = new bool[w * h];
    for (int i = 0; i < diff.Length; i++)
      diff[i] = A(master[i]) > 128 && (A(body[i]) < 128 || Dist(master[i], body[i]) > 70);
    diff = Grow(Grow(diff, w, h, 3, true), w, h, 3, false); // close small gaps (glove seams, outlines)

    // biggest differing shape on each side of her
    var comp = new int[w * h]; var shapes = new List<List<int>>();
    for (int s = 0; s < diff.Length; s++) {
      if (!diff[s] || comp[s] != 0) continue;
      var c = new List<int>(); var st = new Stack<int>(); st.Push(s); comp[s] = shapes.Count + 1;
      while (st.Count > 0) { int i = st.Pop(); c.Add(i); int x = i % w, y = i / w;
        foreach (int nb in new[] { x > 0 ? i - 1 : -1, x < w - 1 ? i + 1 : -1, y > 0 ? i - w : -1, y < h - 1 ? i + w : -1 })
          if (nb >= 0 && diff[nb] && comp[nb] == 0) { comp[nb] = shapes.Count + 1; st.Push(nb); } }
      shapes.Add(c);
    }
    List<int> left = null, right = null;
    foreach (var c in shapes) {
      long sx = 0; foreach (int i in c) sx += i % w; double cx = (double)sx / c.Count / w;
      if (cx < 0.4 && (left == null || c.Count > left.Count)) left = c;
      if (cx > 0.6 && (right == null || c.Count > right.Count)) right = c;
    }
    var report = new List<string>(); var json = new List<string>();
    var cover = new bool[w * h];
    foreach (var side in new[] { Tuple.Create("l", left), Tuple.Create("r", right) }) {
      var c = side.Item2; if (c == null) { report.Add(side.Item1 + ": not found"); continue; }
      var m = new bool[w * h]; foreach (int i in c) m[i] = true;
      // fill holes: anything the outside can't reach
      var outside = new bool[w * h]; var q = new Queue<int>();
      for (int x = 0; x < w; x++) { q.Enqueue(x); q.Enqueue((h - 1) * w + x); }
      for (int y = 0; y < h; y++) { q.Enqueue(y * w); q.Enqueue(y * w + w - 1); }
      while (q.Count > 0) { int i = q.Dequeue(); if (outside[i] || m[i]) continue; outside[i] = true; int x = i % w, y = i / w;
        if (x > 0) q.Enqueue(i - 1); if (x < w - 1) q.Enqueue(i + 1); if (y > 0) q.Enqueue(i - w); if (y < h - 1) q.Enqueue(i + w); }
      for (int i = 0; i < m.Length; i++) if (!outside[i]) m[i] = true;
      // the armless body kept her shoulder caps and gloves, so grow the arm up through skin to the shoulder
      // and down through the glove, within the arm's column, never into hair; then take its outline too
      int x0 = w, x1 = 0, y0 = h, y1 = 0;
      for (int i = 0; i < m.Length; i++) if (m[i]) { x0 = Math.Min(x0, i % w); x1 = Math.Max(x1, i % w); y0 = Math.Min(y0, i / w); y1 = Math.Max(y1, i / w); }
      var grow = new Queue<int>(); var added = new bool[w * h];
      for (int i = 0; i < m.Length; i++) if (m[i] && (i / w < y0 + 5 || i / w > y1 - 5)) grow.Enqueue(i);
      while (grow.Count > 0) { int i = grow.Dequeue(); int x = i % w, y = i / w;
        foreach (int nb in new[] { x > 0 ? i - 1 : -1, x < w - 1 ? i + 1 : -1, y > 0 ? i - w : -1, y < h - 1 ? i + w : -1 }) {
          if (nb < 0 || m[nb] || added[nb]) continue;
          int nx = nb % w, ny = nb / w, cc = master[nb];
          if (nx < x0 - 12 || nx > x1 + 12 || ny < h * 33 / 100 || A(cc) < 200 || Hair(cc)) continue;
          bool up = ny < y0 + 5, ok = up ? Skin(cc) : (Skin(cc) || Dark(cc));
          if (ok) { added[nb] = true; grow.Enqueue(nb); } } }
      var edge = Grow(added, w, h, 2, true);
      for (int i = 0; i < m.Length; i++) if ((added[i] || (edge[i] && Dark(master[i]))) && A(master[i]) > 0) m[i] = true;
      int top = h, bottom = 0; for (int i = 0; i < m.Length; i++) if (m[i]) { top = Math.Min(top, i / w); bottom = Math.Max(bottom, i / w); }
      long px = 0, n = 0; for (int i = 0; i < m.Length; i++) if (m[i] && i / w < top + 40) { px += i % w; n++; }
      double pivX = (double)px / n / w, pivY = (top + 30.0) / h; // shoulder joint, a little below the top
      var arm = new int[w * h];
      for (int i = 0; i < m.Length; i++) if (m[i]) { arm[i] = master[i]; cover[i] = true; }
      Save(arm, w, h, dir + "\\arm_" + side.Item1 + ".png");
      report.Add(string.Format("{0}: {1} px, rows {2}-{3}, pivot {4:0.000},{5:0.000}", side.Item1, c.Count, top, bottom, pivX, pivY));
      json.Add(string.Format(System.Globalization.CultureInfo.InvariantCulture, "\"{0}\": [{1:0.0000}, {2:0.0000}]", side.Item1, pivX, pivY));
    }
    // resting poses without arms: arm areas (a little wider) show the armless body instead
    cover = Grow(cover, w, h, 3, true);
    foreach (var pose in new[] { "master", "talk", "blink" }) {
      var p = Load(dir + "\\" + pose + ".png", out w, out h);
      for (int i = 0; i < p.Length; i++) if (cover[i]) p[i] = body[i];
      Save(p, w, h, dir + "\\" + pose + "_noarms.png");
    }
    System.IO.File.WriteAllText(dir + "\\arms.json", "{ " + string.Join(", ", json) + " }");
    return string.Join("\n", report);
  }
}
"@
$dir = (Resolve-Path $Sprites).Path
[Arms]::Run($dir, "$dir\body.png")
# the keyed body (from key.ps1) was only needed here
Get-ChildItem $dir "body*.png" | ForEach-Object { Remove-Item -LiteralPath $_.FullName -Confirm:$false }
