# Install and build Simplex dependencies in a working directory.
[arg('simplex_bin', long='real', value='simplex', help='Use the installed simplex binary')]
build working_directory simplex_bin='test_simplex':
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    simplex_bin="{{ simplex_bin }}"
    "$simplex_bin" install
    "$simplex_bin" build

# Run Simplex tests in a working directory.
[arg('simplex_bin', long='real', value='simplex', help='Use the installed simplex binary')]
test working_directory simplex_bin='test_simplex':
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    simplex_bin="{{ simplex_bin }}"
    "$simplex_bin" test

# Run Simplex fuzz tests in a working directory.
[arg('simplex_bin', long='real', value='simplex', help='Use the installed simplex binary')]
test_fuzz working_directory simplex_bin='test_simplex':
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    simplex_bin="{{ simplex_bin }}"
    "$simplex_bin" test --fuzz

# Run ordinary tests except Simplex ones in a working directory.
[arg('simplex_bin', long='real', value='simplex', help='Use the installed simplex binary')]
test_no_simplex working_directory simplex_bin='test_simplex':
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    simplex_bin="{{ simplex_bin }}"
    "$simplex_bin" test --no-simplex

# Clean Simplex-generated artifacts in a working directory.
[arg('simplex_bin', long='real', value='simplex', help='Use the installed simplex binary')]
clean working_directory simplex_bin='test_simplex':
    #!/usr/bin/env bash
    set -euo pipefail

    cd -- "{{ working_directory }}"
    simplex_bin="{{ simplex_bin }}"
    "$simplex_bin" clean
