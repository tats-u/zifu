# **ZI**P **F**ile Names to **U**TF-8 (ZIFU)

[![CI (master)](<https://github.com/tats-u/zifu/workflows/CI%20(master)/badge.svg>)](https://github.com/tats-u/zifu/actions/workflows/master.yml)
[![CI (Release)](<https://github.com/tats-u/zifu/workflows/CI%20(Release)/badge.svg>)](https://github.com/tats-u/zifu/actions/workflows/release.yml)

- CLI crate (zifu): [![zifu at crates.io](https://img.shields.io/crates/v/zifu.svg)](https://crates.io/crates/zifu)
[![Crates.io downloads](https://img.shields.io/crates/d/zifu)](https://crates.io/crates/zifu)
[![Crates.io downloads (recent)](https://img.shields.io/crates/dr/zifu)]((https://crates.io/crates/zifu))
- Public API crate (zifu_core): [![zifu at crates.io](https://img.shields.io/crates/v/zifu_core.svg)](https://crates.io/crates/zifu_core)[![zifu_core at docs.rs](https://docs.rs/zifu_core/badge.svg)](https://docs.rs/zifu_core/)
[![Crates.io downloads](https://img.shields.io/crates/d/zifu_core)](https://crates.io/crates/zifu_core)
[![Crates.io downloads (recent)](https://img.shields.io/crates/dr/zifu_core)]((https://crates.io/crates/zifu_core))  

他のOSを使っている人からZIPファイルをもらったけど解凍したらファイル名の日本語が思いっきり文字化け、もしくは他のOSのユーザにZIPファイルを送ったら同じく日本語ファイル名が文字化けしてると言われた・・・そんな経験はありませんか？このツールでは、ZIPファイルのファイル名が全てのOS・言語で文字化けすることなく解凍できる (UTF-8で明示的にエンコードされている) かどうかをチェックし、必要に応じて修復します。

Have you ever received zip files from other OS users and when you decompressed them, their non-English file name were completely garbled, or when you sent zip files to other OS users, you were told that non-English file names were garbled?

This tool checks if file names in zip archives can be decompressed without garbling on all operating systems and languages (i.e. explicitly encoded in UTF-8) and repairs them if not.

## Unicodeの正規化について / For Unicode Normalization

v0.6.0から、一般的なUnicode正規化をされていないUTF-8でエンコーディングされたファイル名を検知・修正するようになりました。  
This tool detects and corrects file names that are encoded in UTF-8 with uncommon Unicode normalizations from the version 0.6.0.

macOSのFinderで作成したZIPファイルは、HFS+独自のUnicode正規化したUTF-8でファイル名をエンコードしています。これをWindows・Linux上で解凍してできたファイルは、以下のような不都合を起こしえます。  
ZIP files created by Finder in macOS contains file names encoded in UTF-8 with the HFS+-specific Unicode normalization.  The files created by extracting them can cause the following inconveniences:

- 一見同名なファイルが複数できてしまう（通常のUnicode正規化をしたファイルと排他しない）  
  Pairs of files that appear to have the same name appear.  They are not treated as duplicated.
- コマンドラインやGUIのテキストボックス等でファイル名・パスを直打ちしても引っかからない  
  You cannot select them by typing their names or paths in CLI or textboxes in GUI.

そのため、（WindowsやLinuxで）一般的な正規化形式になっていないファイル名は、例えUTF-8であることが明示されていたとしても、「ほとんどの環境でファイル名が正しく取り扱われる」とはみなしません。暗黙的な非ASCIIエンコーディングと同様、本ツールでの修正対象とされるべきです。  
Therefore, file names that are not normalized in the common way (in Windows and Linux) should not treated as “dealt with in the most environments,” and c even if it is explicitly specified that they are encoded in UTF-8.  They should be corrected by this tool as with those encoded in implicit non-ASCII encodings.

## インストール / How to install

## バイナリをダウンロード / Download a binary

ここから最新版をダウンロードしてください。 / Download the latest version here.

<https://github.com/tats-u/zifu/releases>

## Cargo

次のコマンドを実行してください。 / Run the following command:

```bash
cargo install zifu
```

## 開発版 / Development version

次のコマンドを実行してください。 / Run the following command:

```bash
cargo install --git https://github.com/tats-u/zifu.git
```

## 使い方

ZIPファイルを修復するには、次のコマンドを入力します。

```text
zifu <ZIPファイルのパス> <出力先のパス>
```

上書きしたい場合は、代わりに次のコマンドを入力します。

```text
zifu -i <ZIPファイルのパス>
```

ZIPファイルが明示的にUTF-8でエンコードされているかどうかをチェックするには、次のコマンドを入力します。

```text
zifu -c <ZIPファイルのパス>
```

ZIPファイルのファイル一覧をチェックするには、次のコマンドを入力します。

```text
zifu -l <ZIPファイルのパス>
```

海外で作成されたZIPファイルの名前を表示・もしくは修復する場合は`-e <エンコーディング>`オプションを使用します。例えば、次のコマンドでアメリカで作成されたZIPファイルのファイル名を表示します。

```text
zifu -l -e cp437 <ZIPファイルのパス>
```

また、非常にレアケースですが、Shift-JISではなく、UTF-8を優先して使用したい場合、`-u`オプションを利用します。

## How to use

To repair a ZIP file, run the following command:

```text
zifu <Path to garbled ZIP file> <Path to output>
```

To overwrite the ZIP file, use the following command instead:

```text
zifu -i <Path to the ZIP file>
```

To check if a ZIP file is explicitly encoded in UTF-8, run the following command:

```text
zifu -c <Path to ZIP file>
```

To list file names in a zip file, rum the following command:

```text
zifu -l <Path to ZIP file>
```

To show file names or repair ZIP archives created outside of your country, add `-e <Encoding>` option.  For example, if you get a ZIP archive from Japan, try:

```text
zifu -e sjis -l <Path to ZIP file>
```

Japanese characters will corrected appear.

If you prefer UTF-8 than the encoding of your language, add `-u` option.  This is important if you speak English, Thai, or Vietnamese.  Encodings of Chinese, Japanese, and Korean usually cannot decode strings encoded in UTF-8 without error, so there is little need to add it if you speak them.

## 制限事項 / Restriction

以下の言語以外非対応です。 / Only these languages are supported:
カッコ内は主要なエンコーディングです。 / Primary encodings are given in parenthesis.

- 日本語 / Japanese (Shift-JIS / EUC-JP)
- 中国語 / Chinese (GBK / BIG5)
- 韓国語 / Korean (EUC-KR)
- ベトナム語 / Vietnamese (Windows-1258)
- タイ語 / Thai (Windows-874)
- 英語 / English (CP437 / CP850)
- 西ヨーロッパ言語 / Western Europe languages (CP850)
- ギリシャ語 / Greek (CP737)
- 中央ヨーロッパ言語 / Central Europe languages (CP852)
- セルビア・ボスニア語 / Serbian & Bosnian (CP855)
- トルコ語など / Turkish etc. (CP857)
- ヘブライ語 / Hebrew (CP862)
- ロシア語など / Russian etc. (CP866)
- アラビア語 / Arabic (CP720)

非対応の言語では、CP437が使用されます。 / CP437 will be used in unsupported languages.

## ライセンス

MITライセンスです。詳しくは[LICENSE.txt](LICENSE.txt)をご覧ください。

## License

The MIT License; see [LICENSE.txt](LICENSE.txt) for details.
