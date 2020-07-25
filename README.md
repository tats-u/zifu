# **ZI**P **F**ile Names to **U**TF-8 (ZIFU)

他のOSを使っている人からZIPファイルをもらったけど解凍したらファイル名の日本語が思いっきり文字化け、もしくは他のOSのユーザにZIPファイルを送ったら同じく日本語ファイル名が文字化けしてると言われた・・・そんな経験はありませんか？このツールでは、ZIPファイルのファイル名が全てのOS・言語で文字化けすることなく解凍できる (UTF-8で明示的にエンコードされている) かどうかをチェックし、必要に応じて修復します。

Have you ever received zip files from other OS users and when you decompressed them, their non-English file name were completely garbled, or when you sent zip files to other OS users, you were told that non-English file names were garbled?

This tool checks if file names in zip archives can be decompressed without garbling on all operating systems and languages (i.e. explicitly encoded in UTF-8) and repairs them if not.

# インストール / How to install

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

# How to use

To repair a ZIP file, run the following command:

```
zifu <Path to garbled ZIP file> <Path to output>
```

To check if a ZIP file is explicitly encoded in UTF-8, run the following command:

```
zifu -c <Path to ZIP file>
```

To list file names in a zip file, ru the following command:

```
zifu -l <ZIPファイルのパス>
```

# ライセンス

MITライセンスです。詳しくは[LICENSE.txt](LICENSE.txt)をご覧ください。

# License

The MIT License; see [LICENSE.txt](LICENSE.txt) for details.