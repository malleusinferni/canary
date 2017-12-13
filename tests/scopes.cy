my $a = 1;

if [] {
    my $a = 2;
} else {
    my $a = 3;
    assert $a eq 3;
    $a = 4;
    assert $a eq 4;
}

assert $a eq 1;
