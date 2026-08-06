What are the shortcomings of with the current setup where the tt is used to build the final proof tree?

GHI
* first-player-loss can be avoided "by not storing any disproofs caused by repetitions"
* current-player-loss: "This scenario does not occur in checkmating problems where only one player’s king is under attack"
* -> GHI not needed?

General
* proof tree is not extractable from tt because evictions
* transpositions are not expanded by dfpn
* transpositions are not easily expanded
  * expansion might end in another transposition
  * recursive transpositions might end in a loop (repetitions)
  * ghi problem?


