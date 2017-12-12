sub main() {
    my $test = returns_nil();
    assert_eq $test, returns_nil();
    assert_eq "nil", str(returns_nil());
}

sub returns_nil() {
    # Empty
}
