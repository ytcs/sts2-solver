Oracle traces of Stratagem reshuffle prompts the simulator does not resume (they replay as UNIMPLEMENTED: the fight is flagged
`missing`, never silently wrong): a draw that is not the last action of a card / potion (Battle Trance, Acrobatics, Prophesize, Bottled
Potential...) and `AutoPlayFromDrawPile` (Mayhem, Cascade, Havoc ...). Not replayed by `cargo test`; a future implementation should
move each pair back to `oracle/regression/`.
