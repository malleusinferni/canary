sub main() {
    my x = 1;

    x = x - 1;

    if x {
        hello(x);
    } else {
        hello("world");
    }
}

sub hello(name) {
    print("Hello, " + name);
}
