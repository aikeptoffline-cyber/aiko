# Cuts accessories out of full poses drawn on the shared canvas, so they can be worn over any pose.
# Each source in art/accessories/<name>.png is the master pose wearing the item; the item is the part that
# differs from art/gen/master.png inside its box, kept where it looks like the item (grey tape, dark lenses).
# Output: ui/sprites/acc-<name>.png on the full canvas, transparent elsewhere, with a soft 2px edge.
param([string]$Src = "$PSScriptRoot\accessories", [string]$Master = "$PSScriptRoot\gen\master.png",
  [string]$Out = "$PSScriptRoot\..\ui\sprites")
$ErrorActionPreference = "Stop"
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System; using System.Drawing; using System.Drawing.Imaging; using System.Runtime.InteropServices;
public static class Acc {
  static int[] Load(string p, int w, int h) {
    var src = new Bitmap(p); var b = new Bitmap(w, h, PixelFormat.Format32bppArgb);
    using (var g = Graphics.FromImage(b)) { g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.HighQualityBicubic; g.DrawImage(src, 0, 0, w, h); }
    src.Dispose(); var px = new int[w * h];
    var d = b.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
    Marshal.Copy(d.Scan0, px, 0, px.Length); b.UnlockBits(d); b.Dispose(); return px;
  }
  static int R(int c) { return (c >> 16) & 255; } static int G(int c) { return (c >> 8) & 255; } static int B(int c) { return c & 255; }
  static int Sat(int c) { return Math.Max(R(c), Math.Max(G(c), B(c))) - Math.Min(R(c), Math.Min(G(c), B(c))); }
  static int Lum(int c) { return (R(c) * 3 + G(c) * 6 + B(c)) / 10; }
  // box: x0,y0,x1,y1 on the 687x1024 canvas; kind: "tape" (grey, low saturation) or "lens" (dark)
  public static void Run(string src, string master, string outp, int x0, int y0, int x1, int y1, string kind) {
    int w = 687, h = 1024; var a = Load(src, w, h); var m = Load(master, w, h);
    var alpha = new int[w * h];
    for (int y = y0; y < y1; y++) for (int x = x0; x < x1; x++) {
      int i = y * w + x, c = a[i], o = m[i];
      int diff = Math.Abs(R(c) - R(o)) + Math.Abs(G(c) - G(o)) + Math.Abs(B(c) - B(o));
      bool item = kind == "tape" ? Sat(c) < 40 && Lum(c) > 60 || Lum(c) < 50 && diff > 60
                                 : Lum(c) < 120 && Lum(c) < Lum(o) - 15 || Lum(c) < 45;
      if (diff > 45 && item) alpha[i] = 255;
    }
    // anything the item encloses is part of it (lens reflections, tape creases): flood the outside from
    // the box border, and whatever it can't reach becomes solid
    var outside = new bool[w * h]; var st = new System.Collections.Generic.Stack<int>();
    for (int x = x0; x < x1; x++) { st.Push(y0 * w + x); st.Push((y1 - 1) * w + x); }
    for (int y = y0; y < y1; y++) { st.Push(y * w + x0); st.Push(y * w + x1 - 1); }
    while (st.Count > 0) {
      int i = st.Pop(), x = i % w, y = i / w;
      if (x < x0 || x >= x1 || y < y0 || y >= y1 || outside[i] || alpha[i] == 255) continue;
      outside[i] = true; st.Push(i - 1); st.Push(i + 1); st.Push(i - w); st.Push(i + w);
    }
    for (int y = y0; y < y1; y++) for (int x = x0; x < x1; x++) { int i = y * w + x; if (!outside[i]) alpha[i] = 255; }
    // fill pinholes, then soften the edge
    var soft = new int[w * h];
    for (int y = y0; y < y1; y++) for (int x = x0; x < x1; x++) {
      int n = 0, s = 0;
      for (int dy = -2; dy <= 2; dy++) for (int dx = -2; dx <= 2; dx++) { int yy = y + dy, xx = x + dx; if (yy < 0 || yy >= h || xx < 0 || xx >= w) continue; n++; s += alpha[yy * w + xx]; }
      int v = s / n; soft[y * w + x] = alpha[y * w + x] == 255 ? (v > 120 ? 255 : v * 2) : (v > 170 ? 255 : v / 2);
    }
    var outPx = new int[w * h];
    for (int i = 0; i < outPx.Length; i++) outPx[i] = soft[i] == 0 ? 0 : (soft[i] << 24) | (a[i] & 0xFFFFFF);
    if (kind == "lens") {
      // frames stay solid; lenses become a see-through tint without the drawn eyes, so her own eyes
      // (and blinks) show through. Frame = dark pixels near the outer edge (distance from outside <= frame).
      const int frame = 9;
      var dist = new int[w * h]; var q = new System.Collections.Generic.Queue<int>();
      for (int y = y0; y < y1; y++) for (int x = x0; x < x1; x++) { int i = y * w + x; if (outside[i]) q.Enqueue(i); else dist[i] = int.MaxValue; }
      while (q.Count > 0) { int i = q.Dequeue(), x = i % w, y = i / w;
        foreach (int nb in new[] { i - 1, i + 1, i - w, i + w }) { int nx = nb % w, ny = nb / w;
          if (nx < x0 || nx >= x1 || ny < y0 || ny >= y1) continue;
          if (dist[nb] > dist[i] + 1) { dist[nb] = dist[i] + 1; q.Enqueue(nb); } } }
      for (int y = y0; y < y1; y++) for (int x = x0; x < x1; x++) {
        int i = y * w + x; if (outside[i] || soft[i] == 0) continue;
        bool isFrame = dist[i] <= frame && Lum(a[i]) < 70 || dist[i] <= 3;
        if (!isFrame) outPx[i] = (Math.Min(soft[i], 180) << 24) | 0x16141E; // dark smoky tint, about 70%
      }
    }
    var o2 = new Bitmap(w, h, PixelFormat.Format32bppArgb);
    var d2 = o2.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
    Marshal.Copy(outPx, 0, d2.Scan0, outPx.Length); o2.UnlockBits(d2); o2.Save(outp, ImageFormat.Png); o2.Dispose();
  }
}
"@
$boxes = @{ tape = @(285, 300, 405, 365, 'tape'); sunglasses = @(236, 226, 462, 314, 'lens') }
foreach ($name in $boxes.Keys) {
  $b = $boxes[$name]; $file = Join-Path $Src "$name.png"
  if (-not (Test-Path $file)) { continue }
  [Acc]::Run((Resolve-Path $file).Path, (Resolve-Path $Master).Path, (Join-Path (Resolve-Path $Out).Path "acc-$name.png"), $b[0], $b[1], $b[2], $b[3], $b[4])
  "acc-$name"
}
