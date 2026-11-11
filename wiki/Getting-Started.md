# Getting Started

## On your profile

1. Create the repository `<username>/<username>` if you do not have it.
2. Add `.github/workflows/dragon.yml`:

```yaml
name: Update Profile Dragon
on:
  schedule:
    - cron: "15 0 * * *"
  workflow_dispatch:
permissions:
  contents: write
jobs:
  update-dragon:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: codewithsyedabdullah/git-dragon@v1
        with:
          window: annual
          branch: output
```

3. Run the workflow once (Actions tab, "Run workflow").
4. Embed the result in your profile `README.md`:

```markdown
![My dragon](https://raw.githubusercontent.com/YOUR_USERNAME/YOUR_USERNAME/output/dragon/dragon.gif)
```

## Locally

```bash
cargo build --release
./target/release/git-dragon demo --commits 800 --languages Python,Go --output-dir dragon
```

Open `dragon/preview.html` to see the GIF on GitHub light and dark backgrounds.
