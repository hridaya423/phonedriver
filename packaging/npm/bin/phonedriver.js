#!/usr/bin/env node
"use strict";
const { main } = require("../lib/launcher");

main(process.argv.slice(2)).then(
  (code) => process.exit(code),
  (err) => {
    process.stderr.write(`phonedriver: ${err.message}\n`);
    process.exit(1);
  },
);
