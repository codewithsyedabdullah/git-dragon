# Troubleshooting

**I see an egg.** The egg means 0 contributions in the selected window. Try `window: rolling`, and remember private contributions only count with a suitable token.

**The GIF does not animate.** Some email clients and viewers show only the first frame. Open `preview.html` or the raw GIF in a browser.

**Wrong colours.** The dragon is tinted by your top three languages by repository count. Use `canon: "true"` for the default emerald look. Repositories without a detected language are skipped.

**Action failed.** Check the log: `collect` errors usually mean a bad username or a token without read access; `commit` errors need `permissions: contents: write`.

**Different size or frame rate.** The canvas (640x300) and the loop (48 frames, 70 ms) are constants in `src/render.rs`.

**Outside GitHub profiles.** The CLI works standalone: `cargo install --git https://github.com/codewithsyedabdullah/git-dragon`.
