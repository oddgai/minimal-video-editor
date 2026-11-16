# アーキテクチャ決定記録（ADR）

このディレクトリには、アーキテクチャを形作り、将来の変更に制約を与える決定を記録します。ADRは決定の理由を残す文書です。実装計画や進捗報告、コードやユーザー向けドキュメントの代わりにはしません。

## 一覧

| ADR | 決定 | 状態 |
| --- | --- | --- |
| [0001](0001-use-tauri-for-the-desktop-app.md) | RustバックエンドとTypeScriptフロントエンドにTauri 2を使う | 採用 |
| [0002](0002-use-keyframe-aligned-stream-copy-for-editing.md) | キーフレーム境界でストリームコピーして編集・書き出しする | 採用 |
| [0003](0003-stream-preview-over-loopback-http.md) | ローカル動画をループバックHTTPで範囲配信してプレビューする | 採用 |
| [0004](0004-preserve-detection-correctness-when-accelerating.md) | チャプター検出の高速化でも全走査の結果を保つ | 採用 |
| [0005](0005-use-audio-as-the-preview-clock-when-offset.md) | 音ズレ補正中は音声をプレビューの基準時計にする | 採用 |
| [0006](0006-bundle-lgpl-ffmpeg.md) | LGPL構成のFFmpegをアプリに同梱する | 採用 |

## 作成ルール

- ADRには連番を付け、ファイル名を `NNNN-short-title.md` にします。
- [template.md](template.md) の構成を使い、一つの長期的な決定に焦点を絞ります。
- Contextには検証可能なプロジェクト上の事実を書きます。根拠となるコード、テスト、設計資料、一次情報をReferencesに記載し、確認日も付けます。
- 決定は方針の粒度で記述します。変化する実装詳細、作業状況、ベンチマーク、移行手順はコード・テスト・計画書・プルリクエストに記録します。
- 重要なトレードオフと代替案を記載します。このADRで決めないことと、その決定を行う場所も明記します。
- 状態には「採用」「提案中」「非推奨」「ADR NNNNで置換」を使います。決定が変わった場合に過去の記録を書き換えず、新しいADRを追加して一覧を更新します。

簡潔な「背景・決定・影響」の構成を使い、検証可能な根拠、明確な範囲、長期的な決定を重視します。[TOKIUMのADR執筆ガイド](https://zenn.dev/tokium_dev/articles/adr-authoring-guideline)（2026-11-16確認）を参考にしつつ、このリポジトリでは単一アプリに必要な範囲だけを採用しています。
