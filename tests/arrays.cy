sub main() {
    my $a = [1, 2, 3, 4];
    assert_eq len($a), 4;
    assert_eq $a[0], 1;
    assert_eq $a[1], 2;
    assert_eq $a[2], 3;
    assert_eq $a[3], 4;

    $a[0] = 3;
    assert_eq $a[0], 3;
}
