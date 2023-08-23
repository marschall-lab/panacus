#!/usr/bin/bash

npm install bootstrap
npm install -g sass
npm install -g css-minify
npm install -g uglify-js 

sass bootstrap.custom.scss bootstrap.custom.css

css-minfy -f bootstrap.custom.css 
uglifyjs lib.js > lib.min.js
uglifyjs hook_after.js > hook-after.min.js

