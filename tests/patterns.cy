my $haystack = "this is a test";

print "Testing the dot";
assert $haystack =~ re/./;

print "Testing \\w";
assert $haystack =~ re/\w/;

print "Testing \\w+";
assert $haystack =~ re/\w+/;

print "Testing \\w{4}";
assert $haystack =~ re/\w{4}/;

print "Match \"this\" at beginning";
assert $haystack =~ re/this is/;

print "Non-initial match";
assert $haystack =~ re/test/;

if 0 {
    # Should parse, but won't succeed... yet
    print $1;
}
