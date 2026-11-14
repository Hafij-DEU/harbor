"""Small pure-Python QR code encoder (byte mode, versions 1-10, ECC level M).

Enough for the share address ("http://192.168.1.20:8765/"). Follows the
ISO/IEC 18004 construction, the same way as Project Nayuki's reference
encoder (MIT). Harbor uses python3-qrcode instead when it is installed.

    matrix(text) -> list of rows, each a list of bools (True = dark)
"""

# level M, versions 1..10
_ECC_PER_BLOCK = (10, 16, 26, 18, 24, 16, 18, 22, 22, 26)
_NUM_BLOCKS = (1, 1, 1, 2, 2, 4, 4, 4, 5, 5)
_FORMAT_M = 0
MAX_VERSION = 10


def _raw_modules(ver):
    n = (16 * ver + 128) * ver + 64
    if ver >= 2:
        na = ver // 7 + 2
        n -= (25 * na - 10) * na - 55
        if ver >= 7:
            n -= 36
    return n


def _data_codewords(ver):
    return _raw_modules(ver) // 8 - _ECC_PER_BLOCK[ver - 1] * _NUM_BLOCKS[ver - 1]


def _gf_mul(x, y):
    z = 0
    for i in reversed(range(8)):
        z = (z << 1) ^ ((z >> 7) * 0x11D)
        z ^= ((y >> i) & 1) * x
    return z


def _rs_divisor(degree):
    res = [0] * (degree - 1) + [1]
    root = 1
    for _ in range(degree):
        for j in range(degree):
            res[j] = _gf_mul(res[j], root)
            if j + 1 < degree:
                res[j] ^= res[j + 1]
        root = _gf_mul(root, 0x02)
    return res


def _rs_remainder(data, divisor):
    res = [0] * len(divisor)
    for b in data:
        factor = b ^ res.pop(0)
        res.append(0)
        for i, coef in enumerate(divisor):
            res[i] ^= _gf_mul(coef, factor)
    return res


def _codewords(data, ver):
    """Data bytes -> final interleaved codeword sequence."""
    cc_bits = 8 if ver <= 9 else 16
    cap = _data_codewords(ver) * 8
    bits = []

    def put(val, n):
        bits.extend((val >> i) & 1 for i in reversed(range(n)))
    put(0b0100, 4)
    put(len(data), cc_bits)
    for b in data:
        put(b, 8)
    put(0, min(4, cap - len(bits)))
    put(0, (-len(bits)) % 8)
    pad = 0xEC
    while len(bits) < cap:
        put(pad, 8)
        pad ^= 0xEC ^ 0x11
    words = [int("".join(map(str, bits[i:i + 8])), 2) for i in range(0, len(bits), 8)]

    nblocks, ecclen = _NUM_BLOCKS[ver - 1], _ECC_PER_BLOCK[ver - 1]
    raw = _raw_modules(ver) // 8
    nshort = nblocks - raw % nblocks
    shortlen = raw // nblocks
    div = _rs_divisor(ecclen)
    blocks, k = [], 0
    for i in range(nblocks):
        dat = words[k:k + shortlen - ecclen + (0 if i < nshort else 1)]
        k += len(dat)
        ecc = _rs_remainder(dat, div)
        if i < nshort:
            dat.append(0)
        blocks.append(dat + ecc)
    out = []
    for i in range(len(blocks[0])):
        for j, blk in enumerate(blocks):
            if i != shortlen - ecclen or j >= nshort:
                out.append(blk[i])
    return out


class _Grid:
    def __init__(self, ver):
        self.ver = ver
        self.size = ver * 4 + 17
        self.mod = [[False] * self.size for _ in range(self.size)]
        self.fn = [[False] * self.size for _ in range(self.size)]

    def setf(self, x, y, dark):
        self.mod[y][x] = dark
        self.fn[y][x] = True

    def align_positions(self):
        if self.ver == 1:
            return []
        na = self.ver // 7 + 2
        step = (self.ver * 8 + na * 3 + 5) // (na * 4 - 4) * 2
        return [6] + sorted(self.size - 7 - i * step for i in range(na - 1))

    def function_patterns(self):
        s = self.size
        for i in range(s):
            self.setf(6, i, i % 2 == 0)
            self.setf(i, 6, i % 2 == 0)
        for cx, cy in ((3, 3), (s - 4, 3), (3, s - 4)):
            for dy in range(-4, 5):
                for dx in range(-4, 5):
                    x, y = cx + dx, cy + dy
                    if 0 <= x < s and 0 <= y < s:
                        self.setf(x, y, max(abs(dx), abs(dy)) not in (2, 4))
        pos = self.align_positions()
        n = len(pos)
        for i in range(n):
            for j in range(n):
                if (i, j) in ((0, 0), (0, n - 1), (n - 1, 0)):
                    continue
                for dy in range(-2, 3):
                    for dx in range(-2, 3):
                        self.setf(pos[i] + dx, pos[j] + dy, max(abs(dx), abs(dy)) != 1)
        self.format_bits(0)
        if self.ver >= 7:
            rem = self.ver
            for _ in range(12):
                rem = (rem << 1) ^ ((rem >> 11) * 0x1F25)
            bits = self.ver << 12 | rem
            for i in range(18):
                bit = (bits >> i) & 1 == 1
                a, b = s - 11 + i % 3, i // 3
                self.setf(a, b, bit)
                self.setf(b, a, bit)

    def format_bits(self, mask):
        data = _FORMAT_M << 3 | mask
        rem = data
        for _ in range(10):
            rem = (rem << 1) ^ ((rem >> 9) * 0x537)
        bits = (data << 10 | rem) ^ 0x5412
        g = [(bits >> i) & 1 == 1 for i in range(15)]
        s = self.size
        for i in range(6):
            self.setf(8, i, g[i])
        self.setf(8, 7, g[6])
        self.setf(8, 8, g[7])
        self.setf(7, 8, g[8])
        for i in range(9, 15):
            self.setf(14 - i, 8, g[i])
        for i in range(8):
            self.setf(s - 1 - i, 8, g[i])
        for i in range(8, 15):
            self.setf(8, s - 15 + i, g[i])
        self.setf(8, s - 8, True)

    def place(self, words):
        s, i, total = self.size, 0, len(words) * 8
        right = s - 1
        while right >= 1:
            if right == 6:
                right = 5
            for vert in range(s):
                for j in range(2):
                    x = right - j
                    up = ((right + 1) & 2) == 0
                    y = s - 1 - vert if up else vert
                    if not self.fn[y][x] and i < total:
                        self.mod[y][x] = (words[i >> 3] >> (7 - (i & 7))) & 1 == 1
                        i += 1
            right -= 2

    def masked(self, mask):
        conds = (lambda x, y: (x + y) % 2 == 0, lambda x, y: y % 2 == 0,
                 lambda x, y: x % 3 == 0, lambda x, y: (x + y) % 3 == 0,
                 lambda x, y: (x // 3 + y // 2) % 2 == 0,
                 lambda x, y: x * y % 2 + x * y % 3 == 0,
                 lambda x, y: (x * y % 2 + x * y % 3) % 2 == 0,
                 lambda x, y: ((x + y) % 2 + x * y % 3) % 2 == 0)
        c = conds[mask]
        self.format_bits(mask)
        return [[self.mod[y][x] ^ (not self.fn[y][x] and c(x, y)) for x in range(self.size)]
                for y in range(self.size)]


def _penalty(m):
    n = len(m)
    score = 0
    lines = m + [list(col) for col in zip(*m)]
    finder_a = "10111010000"
    finder_b = "00001011101"
    for line in lines:
        run, prev = 0, None
        for v in line:
            if v == prev:
                run += 1
            else:
                if run >= 5:
                    score += run - 2
                run, prev = 1, v
        if run >= 5:
            score += run - 2
        s = "0000" + "".join("1" if v else "0" for v in line) + "0000"
        score += 40 * (s.count(finder_a) + s.count(finder_b))
    for y in range(n - 1):
        for x in range(n - 1):
            if m[y][x] == m[y][x + 1] == m[y + 1][x] == m[y + 1][x + 1]:
                score += 3
    dark = sum(map(sum, m))
    total = n * n
    score += 10 * ((abs(dark * 20 - total * 10) + total - 1) // total - 1)
    return score


def matrix(text):
    """QR matrix for text (UTF-8). Raises ValueError when it is too long."""
    data = text.encode("utf-8")
    for ver in range(1, MAX_VERSION + 1):
        cc = 8 if ver <= 9 else 16
        if 4 + cc + 8 * len(data) <= _data_codewords(ver) * 8:
            break
    else:
        raise ValueError("Text too long for a QR code")
    g = _Grid(ver)
    g.function_patterns()
    g.place(_codewords(data, ver))
    best = min((g.masked(k) for k in range(8)), key=_penalty)
    return best


def best_matrix(text):
    """python3-qrcode when installed, else the built-in encoder."""
    try:
        import qrcode
        q = qrcode.QRCode(border=0, error_correction=qrcode.constants.ERROR_CORRECT_M)
        q.add_data(text)
        q.make(fit=True)
        return [list(r) for r in q.get_matrix()]
    except ImportError:
        return matrix(text)
