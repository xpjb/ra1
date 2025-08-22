# Agent
* Rust agent
* use git for checkpoints
* use git show for reviewing its changes
* fees in agent folder too
* Cargo.toml probably always goes in essential context
* oh yea we need to use go to definition as well, maybe just grep for the definition
* may want some cargo fix, cargo ignore warnings or quick fix tried if it says 'Hint' etc

## Reader
* recurses working directory
* thinks into .agent folder 
* index: filename - summary
* history: history of commands and thoughts
* Initial mode, also have an updater mode that does it based on last diff

## Executive System
* ask important questions for more info
* Formulates strategy
* Dispatches other systems
* Cargo check: loop the writer again (just with the errors), maybe some small local models
* max tries can be a flag too

## Gatherer
* Build relevant context for programming goal by reading index
* cat relevant sections and stitch with known format eg patch format or something if theres a known segment format

## Writer
* Takes its optimized context and goal
* blast the new code out

# Tester


* Executive System
- internal monologue, plans how to fulfill the users request, decomposing into a procedure
 - interpret request
 - gather missing information
 - prepare context
 - previous context
- get context of what files to modify and what code is implicated
* Context Getter
 - what context is missing? read files, grep files, search web. go to definition equivalent
 - debuggin - use cargo check output
* Code Writer
 - responds with its best guess at how to fulfill the changes

* .agent files
* file system understander and updater
 - ls -l
 - .agent folder


 -----------

 Yea i reckon it needs like an agent subprocess to wait on or whatever, wait til its done get its response, etc maybe spinner, ttl / timer etc

 interactive mode would use recent changes to catch bugs
 praying that its trained on diffs lol

 yea debugging is kinda like literally here are all the different bug states, they need to be compressed and labelled. then you could make the conclusion
