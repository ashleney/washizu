# washizu

WIP project to show hidden board state to understand why Mortal recommended a specific move.

Mostly meant for tile efficiency and non-obvious guaranteed tile safety.

[Mortal is optimized for playing, not reviewing or attribution](https://github.com/Equim-chan/mjai-reviewer/blob/master/faq.md#mortal-why-do-all-actions-except-the-best-sometimes-have-significantly-lower-q-values-than-that-of-the-best), but that doesn't mean we can't try and force a reason out of it. Most decisions can be attributed to pure tile efficiency and safety. For instances where this does not apply, an alteration of mortal's playerstate (such as changing the tehai or kawa) could yield more insight. (this will however require a weaker local engine...)

This project goes fundamentally against the goal Mortal was designed for, but because model-free engines are currently dominating over typical engines, attempting to analyze mortal's output should end up being more valuable.

The project significantly sacrifices performance since it chooses to calculate expected value tables for all hands instead of up to 3-shanten.

## Credits
- [Equim-chan/Mortal](https://github.com/Equim-chan/Mortal) - AI for riichi mahjong.
- [shinkuan](https://github.com/shinkuan) - Provider of a public pre-trained Mortal model.
- [killerducky/killer_mortal_gui](https://github.com/killerducky/killer_mortal_gui) - Mortal GUI and defense calculation.
- [EndlessCheng/mahjong-helper](https://github.com/EndlessCheng/mahjong-helper) - Old defense calculation.
- [The Hopeless Girl on the Path of Houou](https://pathofhouou.blogspot.com/) - Replay analysis of Tenhou games.

## Unexpected behavior with no clear solution
- Yaku names are localized to be recognizable specifically to the author
- Houtei is not calculated and haitei is overvalued, e.g. open hand 234m4p111222333z chi 1m dahai 4p may be valued 50 points higher if it makes us the candidate for haitei
- Agari off Ron is not calculated which causes open hands to lose more points than expected. A chance of agari being Ron should be included in calculation.
- Tsumo-only causes furiten to not be considered. It is also extra state that would mess with the cache.
