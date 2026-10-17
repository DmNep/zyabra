"""Build the embedded word lists from already downloaded sources.

The running program never calls this and never uses the network.
Run it by hand after placing the source files in third_party/:

  third_party/enable1.txt            ENABLE, public domain
  third_party/russian-cp1251.txt     https://github.com/danakt/russian-words (MIT), cp1251
"""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
THIRD = ROOT / "third_party"
OUT = ROOT / "dictionaries"


def english_words() -> set[str]:
    words: set[str] = set()
    for line in (THIRD / "enable1.txt").read_text(encoding="utf-8").splitlines():
        word = line.strip().lower()
        if 2 <= len(word) <= 20 and word.isascii() and word.isalpha():
            words.add(word)
    return words


def russian_words() -> set[str]:
    text = (THIRD / "russian-cp1251.txt").read_bytes().decode("cp1251")
    words: set[str] = set()
    for line in text.splitlines():
        word = line.strip().lower().replace("ё", "е")
        if 2 <= len(word) <= 20 and all("а" <= ch <= "я" for ch in word):
            words.add(word)
    return words


def write(path: Path, words: set[str]) -> None:
    path.write_text("\n".join(sorted(words)) + "\n", encoding="utf-8", newline="\n")


def main() -> None:
    OUT.mkdir(exist_ok=True)
    en = english_words()
    ru = russian_words()
    write(OUT / "en.txt", en)
    write(OUT / "ru.txt", ru)
    print(f"en {len(en)}")
    print(f"ru {len(ru)}")


if __name__ == "__main__":
    main()
