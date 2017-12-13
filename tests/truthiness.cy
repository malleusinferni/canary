assert 1;
assert 0-1;

if [] {
    assert 0;
}

assert new();

assert 1 eq 1;
assert 1 ne 0;
assert [] ne 4;

assert :a eq :a;

assert 1 and 1;

assert 1 or 0;
assert 0 or 1;
