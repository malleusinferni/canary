my $x = 4;

while $x {
    $x = $x - 1;
    hello "world";
}

sub hello($name) {
    print "Hello, ", $name;
}
