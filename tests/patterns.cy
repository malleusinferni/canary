my $needle = re/test/;
my $haystack = "this is a test";
assert $haystack =~ $needle;

if 0 {
    # Should parse, but won't succeed... yet
    print $1;
}
