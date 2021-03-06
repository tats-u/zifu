# **ZI**P **F**ile Names to **U**TF-8 (ZIFU)

![CI (master)](<https://github.com/tats-u/zifu/workflows/CI%20(master)/badge.svg>)
![CI (Release)](<https://github.com/tats-u/zifu/workflows/CI%20(Release)/badge.svg>)
[![zifu at crates.io](https://img.shields.io/crates/v/zifu.svg)](https://crates.io/crates/zifu)

他のOSを使っている人からZIPファイルをもらったけど解凍したらファイル名の日本語が思いっきり文字化け、もしくは他のOSのユーザにZIPファイルを送ったら同じく日本語ファイル名が文字化けしてると言われた・・・そんな経験はありませんか？このツールでは、ZIPファイルのファイル名が全てのOS・言語で文字化けすることなく解凍できる (UTF-8で明示的にエンコードされている) かどうかをチェックし、必要に応じて修復します。

Have you ever received zip files from other OS users and when you decompressed them, their non-English file name were completely garbled, or when you sent zip files to other OS users, you were told that non-English file names were garbled?


This tool checks if file names in zip archives can be decompressed without garbling on all operating systems and languages (i.e. explicitly encoded in UTF-8) and repairs them if not.
# インストール / How to install

## バイナリをダウンロード / Download a binary

ここから最新版をダウンロードしてください。 / Download the latest version here.

https://github.com/tats-u/zifu/releases

## Cargo

次のコマンドを実行してください。 / Run the following command:

```
cargo install zifu
```

## 開発版 / Development version

次のコマンドを実行してください。 / Run the following command:

```
cargo install --git https://github.com/tats-u/zifu.git
```

# 使い方

ZIPファイルを修復するには、次のコマンドを入力します。

```
zifu <ZIPファイルのパス> <出力先のパス>
```

ZIPファイルが明示的にUTF-8でエンコードされているかどうかをチェックするには、次のコマンドを入力します。

```
zifu -c <ZIPファイルのパス>
```

ZIPファイルのファイル一覧をチェックするには、次のコマンドを入力します。

```
zifu -l <ZIPファイルのパス>
```

海外で作成されたZIPファイルの名前を表示・もしくは修復する場合は`-e <エンコーディング>`オプションを使用します。例えば、次のコマンドでアメリカで作成されたZIPファイルのファイル名を表示します。

```
zifu -l -e cp437 <ZIPファイルのパス>
```

また、非常にレアケースですが、Shift-JISではなく、UTF-8を優先して使用したい場合、`-u`オプションを利用します。

# How to use

To repair a ZIP file, run the following command:

```
zifu <Path to garbled ZIP file> <Path to output>
```

To check if a ZIP file is explicitly encoded in UTF-8, run the following command:

```
zifu -c <Path to ZIP file>
```

To list file names in a zip file, rum the following command:

```
zifu -l <Path to ZIP file>
```

To show file names or repair ZIP archives created outside of your country, add `-e <Encoding>` option.  For example, if you get a ZIP archive from Japan, try:

```
zifu -e sjis -l <Path to ZIP file>
```

Japanese characters will corrected appear.

If you prefer UTF-8 than the encoding of your language, add `-u` option.  This is important if you speak English, Thai, or Vietnamese.  Encodings of Chinese, Japanese, and Korean usually cannot decode strings encoded in UTF-8 without error, so there is little need to add it if you speak them.

# 制限事項 / Restriction

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

# ライセンス

MITライセンスです。詳しくは[LICENSE.txt](LICENSE.txt)をご覧ください。

# License

The MIT License; see [LICENSE.txt](LICENSE.txt) for details.