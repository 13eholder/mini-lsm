#!/bin/bash

mdbook build
sscli -b https://13eholder.github.io/mini-lsm -r book -f xml -o > src/sitemap.xml
sscli -b https://13eholder.github.io/mini-lsm -r book -f txt -o > src/sitemap.txt
