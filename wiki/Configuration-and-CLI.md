# Configuration and CLI

## Action inputs

| Input | Default | Description |
|---|---|---|
| `login` | repository owner | Account to draw. |
| `window` | `annual` | `annual` or `rolling`. |
| `canon` | `"false"` | Ignore language colours. |
| `output-dir` | `dragon` | Output folder. |
| `commit` | `"true"` | Publish to `branch`. |
| `branch` | `output` | Target branch. |
| `token` | `github.token` | Token for reading data and pushing. |

## Commands

```
git-dragon generate <login> [--window annual|rolling] [--output-dir dragon] [--canon]
git-dragon collect  <login> [--window annual|rolling] [--output snapshot.json]
git-dragon render   <snapshot.json> [--output-dir dragon] [--canon]
git-dragon demo     [--commits 2800] [--languages A,B,C] [--output-dir dragon] [--canon]
git-dragon gallery  [--languages A,B,C] [--output-dir dragon] [--canon]
```

`GITHUB_TOKEN` (or `GH_TOKEN`) enables the GraphQL path and private-contribution counts. Output: `dragon.gif`, `dragon.png`, `share-card.png`, `preview.html`. `gallery` writes one GIF per stage (`egg.gif` ... `legendary.gif`).
