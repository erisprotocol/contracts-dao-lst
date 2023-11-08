#!/usr/bin/env bash

set -e
set -o pipefail

projectPath=$(cd "$(dirname "${0}")" && cd ../ && pwd)

for c in "$projectPath"/contracts/*; do
  if [[ "$c" != *"proxies" ]]; then
    if [[ "$c" != *"amp-governance" ]]; then
      if [[ "$c" != *"arb-vault" ]]; then
        (cd $c && cargo schema)
      fi
    fi
  fi
done

for c in "$projectPath"/contracts/proxies/*; do
  if [[ "$c" != *"README.md" ]]; then
    (cd $c && cargo schema)
  fi
done

for c in "$projectPath"/contracts/amp-governance/*; do
  if [[ "$c" != *"README.md" ]]; then
    (cd $c && cargo schema)
  fi
done

# for c in "$projectPath"/contracts/periphery/*; do
#   (cd $c && cargo schema)
# done
