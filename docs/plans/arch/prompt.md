I want a clear separation of the following concerns:

1. move generation: cleanly outsourced to `atomic-movegen` dependency
2. search for outcome: dfpn implementation
3. providing proof: export via proof-tree

Point 1 is solved. I am not sure about the clean separation of 2 and 3 though.

Of course the search emits nodes to the proof tree via the worker, this is fine.
But for a clean architecture the proof-tree must have no knowledge of the search and the search must only know the worker, not the proof-tree itself.

This is just brainstorming at the moment, help me challenge these ideas.
