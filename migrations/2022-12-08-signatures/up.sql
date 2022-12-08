alter table crates
    add column signature text;

comment on column crates.signature is 'NULL or a signature of the crate';
