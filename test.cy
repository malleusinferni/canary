sub main() {
    my x = 4;
    while x {
        hello("world");
        x = x - 1;
    }
}

sub hello(name) {
    print("Hello, " + name);
}
