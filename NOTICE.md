# Third-party word lists

Zyabra embeds two word lists. The program does not download them.

## English

ENABLE (Enhanced North American Benchmark Lexicon), public domain.
Filtered to words of 2–20 letters. Regeneration script: `tools/build_dicts.py`.
A commonly mirrored copy is <https://github.com/dolph/dictionary> (`enable1.txt`).

## Russian

Derived from <https://github.com/danakt/russian-words> (`russian.txt`), MIT License.
Copyright (c) 2016 danakt.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

The shipped `dictionaries/ru.txt` is that list lowercased, with `ё` folded to `е`,
restricted to words of 2–20 Cyrillic letters, then sorted.
