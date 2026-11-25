# Keys the green-screen poses in art/gen (all on one shared canvas, so they stay aligned) into transparent
# PNGs in ui/sprites: green removed, green spill removed from edges, alpha faded over the outer `Feather` px.
# Each pose also gets <name>-iris.png (irises only) and <name>-eyes.png (where irises may move), and the
# irises in <name>.png are painted over with eye white, so the app can make her eyes follow the cursor.
param([string]$In = "$PSScriptRoot\gen", [string]$Out = "$PSScriptRoot\..\ui\sprites", [int]$Feather = 5)
$ErrorActionPreference = "Stop"
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System; using System.Collections.Generic; using System.Drawing; using System.Drawing.Imaging;
using System.Runtime.InteropServices;
public static class Key {
  static int R(int c) { return (c >> 16) & 255; } static int G(int c) { return (c >> 8) & 255; } static int B(int c) { return c & 255; }
  static bool Iris(int c) { return G(c) > R(c) + 25 && B(c) > R(c) + 35 && B(c) > 80; } // teal, to find the eyes
  static bool White(int c) { int mn = Math.Min(R(c), Math.Min(G(c), B(c))), mx = Math.Max(R(c), Math.Max(G(c), B(c))); return mn >= 150 && mx - mn <= 45; }

  static void Save(int[] px, int w, int h, string path) {
    var o = new Bitmap(w, h, PixelFormat.Format32bppArgb);
    var d = o.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
    Marshal.Copy(px, 0, d.Scan0, px.Length); o.UnlockBits(d); o.Save(path, ImageFormat.Png); o.Dispose();
  }

  public static void Run(string src, string outBase, int feather) {
    var bmp = new Bitmap(src); int w = bmp.Width, h = bmp.Height;
    var px = new int[w * h];
    var d = bmp.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
    Marshal.Copy(d.Scan0, px, 0, px.Length); bmp.UnlockBits(d); bmp.Dispose();

    // chroma key: how much green beats red/blue decides alpha; spill is clamped off the colour
    var a = new int[w * h];
    for (int i = 0; i < px.Length; i++) {
      int c = px[i], r = R(c), g = G(c), b = B(c), spill = g - Math.Max(r, b);
      a[i] = spill <= 25 ? 255 : spill >= 90 ? 0 : 255 - (spill - 25) * 255 / 65;
      if (spill > 0) g = Math.Max(r, b);
      px[i] = (r << 16) | (g << 8) | b;
    }
    // feather: fade alpha over the outermost `feather` px, except along the bottom edge (the waist cut)
    var dist = new int[w * h]; var q = new Queue<int>();
    for (int i = 0; i < px.Length; i++) {
      if (a[i] == 0) continue; int x = i % w, y = i / w; dist[i] = int.MaxValue;
      bool edge = x == 0 || x == w - 1 || y == 0 || a[i - 1] == 0 || a[i + 1] == 0 || a[i - w] == 0 || (y < h - 1 && a[i + w] == 0);
      if (edge && y < h - 1) { dist[i] = 1; q.Enqueue(i); }
    }
    while (q.Count > 0) { int i = q.Dequeue(); if (dist[i] >= feather) continue; int x = i % w, y = i / w;
      foreach (int nb in new[] { x > 0 ? i - 1 : -1, x < w - 1 ? i + 1 : -1, y > 0 ? i - w : -1, y < h - 1 ? i + w : -1 })
        if (nb >= 0 && dist[nb] > dist[i] + 1) { dist[nb] = dist[i] + 1; q.Enqueue(nb); } }
    for (int i = 0; i < px.Length; i++) if (dist[i] > 0 && dist[i] <= feather) a[i] = a[i] * dist[i] / (feather + 1);

    // eyes: eye-sized teal blobs locate them; the iris is then everything lying between eye-white pixels on
    // the same row (ring, pupil), plus highlights enclosed by that
    var seen = new bool[w * h]; var eyes = new List<Rectangle>();
    for (int s = 0; s < px.Length; s++) {
      if (seen[s] || a[s] < 255 || !Iris(px[s])) continue;
      int x0 = w, y0 = h, x1 = 0, y1 = 0, n = 0; var st = new Stack<int>(); st.Push(s); seen[s] = true;
      while (st.Count > 0) { int i = st.Pop(); n++; int x = i % w, y = i / w;
        x0 = Math.Min(x0, x); y0 = Math.Min(y0, y); x1 = Math.Max(x1, x); y1 = Math.Max(y1, y);
        foreach (int nb in new[] { i - 1, i + 1, i - w, i + w })
          if (nb >= 0 && nb < px.Length && !seen[nb] && a[nb] == 255 && Iris(px[nb])) { seen[nb] = true; st.Push(nb); } }
      var r = Rectangle.FromLTRB(x0, y0, x1 + 1, y1 + 1);
      if (n < 15 || r.Width > 60 || r.Height > 60) continue;
      int k = eyes.FindIndex(e => { var f = e; f.Inflate(6, 6); return f.IntersectsWith(r); });
      if (k >= 0) eyes[k] = Rectangle.Union(eyes[k], r); else eyes.Add(r);
    }
    var basePx = new int[w * h]; var irisPx = new int[w * h]; var maskPx = new int[w * h];
    for (int i = 0; i < px.Length; i++) basePx[i] = (a[i] << 24) | px[i];
    foreach (var e in eyes) {
      var area = e; area.Inflate(14, 8);
      area.Intersect(new Rectangle(0, 0, w, h));
      var core = e; core.Inflate(3, 1); // the iris itself; rows above and below it are lashes and lids
      var iris = new HashSet<int>(); var whites = new HashSet<int>();
      for (int y = area.Top; y < area.Bottom; y++) {
        int lo = -1, hi = -1;
        for (int x = area.Left; x < area.Right; x++) if (White(px[y * w + x])) { if (lo < 0) lo = x; hi = x; }
        if (lo < 0 || hi - lo < 4) continue;
        for (int x = lo; x <= hi; x++) {
          int i = y * w + x;
          if (White(px[i])) whites.Add(i); else if (core.Contains(x, y)) iris.Add(i);
        }
      }
      // highlights inside the iris move with it
      foreach (int i in new List<int>(whites)) {
        bool left = false, right = false;
        for (int k = 1; k <= 5; k++) { if (iris.Contains(i - k)) left = true; if (iris.Contains(i + k)) right = true; }
        if (left && right) { whites.Remove(i); iris.Add(i); }
      }
      long wr = 0, wg = 0, wb = 0, wn = 0;
      foreach (int i in whites) { wr += R(px[i]); wg += G(px[i]); wb += B(px[i]); wn++; }
      int white = wn > 0 ? (int)(wr / wn) << 16 | (int)(wg / wn) << 8 | (int)(wb / wn) : 0xF0F4FA;
      foreach (int i in iris) { irisPx[i] = unchecked((int)0xFF000000) | px[i]; basePx[i] = unchecked((int)0xFF000000) | white; maskPx[i] = -1; }
      foreach (int i in whites) maskPx[i] = -1;
    }
    Save(basePx, w, h, outBase + ".png");
    Save(irisPx, w, h, outBase + "-iris.png");
    Save(maskPx, w, h, outBase + "-eyes.png");
  }
}
"@
New-Item -ItemType Directory -Force $Out | Out-Null
$outDir = (Resolve-Path $Out).Path
Get-ChildItem $In *.png | ForEach-Object { [Key]::Run($_.FullName, (Join-Path $outDir $_.BaseName), $Feather); $_.BaseName }
