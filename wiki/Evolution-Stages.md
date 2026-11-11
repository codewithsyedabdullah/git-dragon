# Evolution Stages

| Stage | Contributions | Name | Fire breath |
|---|---|---|---|
| 0 | 0 | Dragon Egg | no |
| 1 | 1 - 149 | Baby Dragon | no |
| 2 | 150 - 499 | Young Dragon | no |
| 3 | 500 - 1,199 | Adolescent Dragon | small |
| 4 | 1,200 - 2,499 | Adult Dragon | medium |
| 5 | 2,500+ | Legendary Dragon | large |

The thresholds live in `src/stage.rs` (`Stage::from_contributions`) and are covered by unit tests at every boundary. The count is the larger of the window total and the monthly total from the snapshot. By default the window is the current calendar year, so every dragon returns to an egg on January 1st; `--window rolling` uses the trailing 365 days.

All stages share one design language: emerald scales, turquoise highlights, cream belly and horns, dark outlines. Wings grow from tiny to full-span, horns lengthen, the tail gains a crest, and the legendary dragon adds an aura, golden wing edges, embers and stars.

Animation per stage (48-frame loop, about 3.4 s):

- Egg: slow glint.
- Baby to Legendary: three wing flaps per loop (swing grows with the stage) and two blinks.
- Adolescent, Adult, Legendary: one fire breath per loop.
- Legendary: rising embers and twinkling stars.
