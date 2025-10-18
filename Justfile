test:
    #!/usr/bin/env bash
    CHIPS=(
        stm32f407ie
        stm32f407ig
        stm32f407ve
        stm32f407vg
        stm32f407ze
        stm32f407zg
    )

    for chip in "${CHIPS[@]}"; do
        echo "CHIP: $chip"
        (cd example; cargo build --no-default-features --features "embassy-stm32/$chip")
    done

release *args:
    (cd lib; jj run-git cargo release --no-verify --tag-prefix= {{args}})
