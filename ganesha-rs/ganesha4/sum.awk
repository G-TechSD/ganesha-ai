#!/usr/bin/awk -f
# sum.awk – sums all numeric fields in the input

{
    for (i = 1; i <= NF; i++) {
        if ($i ~ /^[0-9]+(\.[0-9]+)?$/) {
            total += $i
        }
    }
}

END {
    printf "Total: %.2f\n", total
}
