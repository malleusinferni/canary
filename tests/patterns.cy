my $haystack = "this is a test";

print "Testing the dot";
assert $haystack =~ re/./;

print "Testing \\w";
assert $haystack =~ re/\w/;

print "Testing \\w+";
assert $haystack =~ re/\w+/;

print "Testing \\w{3}";
assert $haystack =~ re/\w{3}/;

print "Testing \\w{4}";
assert $haystack =~ re/\w{4}/;

print "Match \"this\" at beginning";
assert $haystack =~ re/this is/;

print "Non-initial match";
assert $haystack =~ re/test/;

print "Anchored match";
assert $haystack =~ re/^this is a test$/;

print "Case-insensitive";
assert $haystack =~ re/THIS IS A TEST/i;

print "Numbered captures";
$haystack =~ re/(this) (is) (a) (test)/;
assert $0 eq $haystack;
assert $1 eq "this";
assert $2 eq "is";
assert $3 eq "a";
assert $4 eq "test";
