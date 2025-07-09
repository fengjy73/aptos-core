
<a id="0x1_xtl_kitty_breeding"></a>

# Module `0x1::xtl_kitty_breeding`



-  [Struct `Kitty`](#0x1_xtl_kitty_breeding_Kitty)
-  [Struct `KittyInfo`](#0x1_xtl_kitty_breeding_KittyInfo)
-  [Resource `MyKitties`](#0x1_xtl_kitty_breeding_MyKitties)
-  [Resource `AllKitties`](#0x1_xtl_kitty_breeding_AllKitties)
-  [Resource `Newborns`](#0x1_xtl_kitty_breeding_Newborns)
-  [Function `initialize`](#0x1_xtl_kitty_breeding_initialize)
-  [Function `mint`](#0x1_xtl_kitty_breeding_mint)
-  [Function `breed`](#0x1_xtl_kitty_breeding_breed)
-  [Function `create`](#0x1_xtl_kitty_breeding_create)
-  [Function `add_new_kitty`](#0x1_xtl_kitty_breeding_add_new_kitty)
-  [Function `sqrt`](#0x1_xtl_kitty_breeding_sqrt)
-  [Function `genes_mix`](#0x1_xtl_kitty_breeding_genes_mix)


<pre><code><b>use</b> <a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">0x1::signer</a>;
<b>use</b> <a href="../../aptos-stdlib/doc/table.md#0x1_table">0x1::table</a>;
</code></pre>



<a id="0x1_xtl_kitty_breeding_Kitty"></a>

## Struct `Kitty`



<pre><code><b>struct</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_Kitty">Kitty</a> <b>has</b> drop, store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>id: u64</code>
</dt>
<dd>

</dd>
<dt>
<code>genes: u64</code>
</dt>
<dd>

</dd>
<dt>
<code>m_id: u64</code>
</dt>
<dd>

</dd>
<dt>
<code>s_id: u64</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a id="0x1_xtl_kitty_breeding_KittyInfo"></a>

## Struct `KittyInfo`



<pre><code><b>struct</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_KittyInfo">KittyInfo</a> <b>has</b> drop, store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>gender: bool</code>
</dt>
<dd>

</dd>
<dt>
<code>owner: <b>address</b></code>
</dt>
<dd>

</dd>
</dl>


</details>

<a id="0x1_xtl_kitty_breeding_MyKitties"></a>

## Resource `MyKitties`



<pre><code><b>struct</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_MyKitties">MyKitties</a> <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>value: <a href="../../aptos-stdlib/doc/table.md#0x1_table_Table">table::Table</a>&lt;u64, <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_Kitty">xtl_kitty_breeding::Kitty</a>&gt;</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a id="0x1_xtl_kitty_breeding_AllKitties"></a>

## Resource `AllKitties`



<pre><code><b>struct</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_AllKitties">AllKitties</a> <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>value: <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector">vector</a>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_KittyInfo">xtl_kitty_breeding::KittyInfo</a>&gt;</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a id="0x1_xtl_kitty_breeding_Newborns"></a>

## Resource `Newborns`



<pre><code><b>struct</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_Newborns">Newborns</a> <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>value: u64</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a id="0x1_xtl_kitty_breeding_initialize"></a>

## Function `initialize`



<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_initialize">initialize</a>(<b>global</b>: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_initialize">initialize</a>(<b>global</b>: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>){
    <b>move_to</b>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_AllKitties">AllKitties</a>&gt;(<b>global</b>,<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_AllKitties">AllKitties</a>{
        value: <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_empty">vector::empty</a>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_KittyInfo">KittyInfo</a>&gt;()
    });
    <b>move_to</b>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_Newborns">Newborns</a>&gt;(<b>global</b>,<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_Newborns">Newborns</a>{
        value: 0
    });
}
</code></pre>



</details>

<a id="0x1_xtl_kitty_breeding_mint"></a>

## Function `mint`



<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_mint">mint</a>(owner: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>, <b>global</b>: <b>address</b>, genes: u64, gender: bool)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_mint">mint</a>(owner: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>, <b>global</b>:<b>address</b>, genes:u64, gender:bool)<b>acquires</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_AllKitties">AllKitties</a>,<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_MyKitties">MyKitties</a>{
    <b>let</b> addr = <a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer_address_of">signer::address_of</a>(owner);
    <b>if</b>(!<b>exists</b>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_MyKitties">MyKitties</a>&gt;(addr)){
        <b>move_to</b>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_MyKitties">MyKitties</a>&gt;(owner,<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_MyKitties">MyKitties</a>{
            value: <a href="../../aptos-stdlib/doc/table.md#0x1_table_new">table::new</a>()
        });
    };
    <b>let</b> id = <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_create">create</a>(<b>global</b>,gender,addr);
    <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_add_new_kitty">add_new_kitty</a>(addr,genes,id,((1&lt;&lt;32)-1 <b>as</b> u64), ((1&lt;&lt;32)-1 <b>as</b> u64) );
}
</code></pre>



</details>

<a id="0x1_xtl_kitty_breeding_breed"></a>

## Function `breed`



<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_breed">breed</a>(sender: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>, <b>global</b>: <b>address</b>, m: u64, s: u64, gender: bool)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_breed">breed</a>(sender: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>, <b>global</b>:<b>address</b>, m:u64, s:u64, gender:bool) <b>acquires</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_MyKitties">MyKitties</a>,<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_AllKitties">AllKitties</a>,<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_Newborns">Newborns</a>{
    <b>let</b> new_owner = <a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer_address_of">signer::address_of</a>(sender);
    <b>let</b> all_kitties = <b>borrow_global_mut</b>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_AllKitties">AllKitties</a>&gt;(<b>global</b>);
    <b>let</b> m_owner = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow_mut">vector::borrow_mut</a>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_KittyInfo">KittyInfo</a>&gt;(&<b>mut</b> all_kitties.value,m).owner;
    <b>let</b> s_owner = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow_mut">vector::borrow_mut</a>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_KittyInfo">KittyInfo</a>&gt;(&<b>mut</b> all_kitties.value,s).owner;
    <b>let</b> m_genes:u64;
    {
        <b>let</b> _my_kitties_m = <b>borrow_global_mut</b>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_MyKitties">MyKitties</a>&gt;(m_owner);
        m_genes = <a href="../../aptos-stdlib/doc/table.md#0x1_table_borrow_mut">table::borrow_mut</a>(&<b>mut</b> _my_kitties_m.value,m).genes;
    };
    <b>let</b> s_genes:u64;
    {
        <b>let</b> _my_kitties_s = <b>borrow_global_mut</b>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_MyKitties">MyKitties</a>&gt;(s_owner);
        s_genes = <a href="../../aptos-stdlib/doc/table.md#0x1_table_borrow_mut">table::borrow_mut</a>(&<b>mut</b> _my_kitties_s.value,s).genes;
    };
    <b>let</b> new_genes = <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_genes_mix">genes_mix</a>(m_genes,s_genes);
    <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_add_new_kitty">add_new_kitty</a>(new_owner, new_genes,new_genes, m, s);
    <b>let</b> newborns = &<b>mut</b> <b>borrow_global_mut</b>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_Newborns">Newborns</a>&gt;(<b>global</b>).value;
    *newborns = *newborns + 1;
}
</code></pre>



</details>

<a id="0x1_xtl_kitty_breeding_create"></a>

## Function `create`



<pre><code><b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_create">create</a>(<b>global</b>: <b>address</b>, gender: bool, owner: <b>address</b>): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_create">create</a>(<b>global</b>:<b>address</b>, gender: bool, owner: <b>address</b>):u64 <b>acquires</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_AllKitties">AllKitties</a>{
    <b>let</b> all_kitties = <b>borrow_global_mut</b>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_AllKitties">AllKitties</a>&gt;(<b>global</b>);
    <b>let</b> id = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_length">vector::length</a>(&all_kitties.value);
    <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_push_back">vector::push_back</a>(&<b>mut</b> all_kitties.value, <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_KittyInfo">KittyInfo</a>{
        gender,
        owner
    });
    id
}
</code></pre>



</details>

<a id="0x1_xtl_kitty_breeding_add_new_kitty"></a>

## Function `add_new_kitty`



<pre><code><b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_add_new_kitty">add_new_kitty</a>(owner: <b>address</b>, genes: u64, id: u64, m_id: u64, s_id: u64)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_add_new_kitty">add_new_kitty</a>(owner: <b>address</b>, genes:u64, id:u64,m_id:u64, s_id:u64) <b>acquires</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_MyKitties">MyKitties</a>{
    <b>let</b> my_kitties = <b>borrow_global_mut</b>&lt;<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_MyKitties">MyKitties</a>&gt;(owner);
    <a href="../../aptos-stdlib/doc/table.md#0x1_table_add">table::add</a>(&<b>mut</b> my_kitties.value, id, <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_Kitty">Kitty</a>{
        id,
        genes,
        m_id,
        s_id
    });
}
</code></pre>



</details>

<a id="0x1_xtl_kitty_breeding_sqrt"></a>

## Function `sqrt`



<pre><code><b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(x: u64): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(x:u64):u64 {
    <b>let</b> z = (x+1)/2;
    <b>let</b> y = x;
    <b>while</b> (z &lt; y){
        y = z;
        z = (x/z+z)/2;
    };
    y
}
</code></pre>



</details>

<a id="0x1_xtl_kitty_breeding_genes_mix"></a>

## Function `genes_mix`



<pre><code><b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_genes_mix">genes_mix</a>(m_genes: u64, s_genes: u64): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_genes_mix">genes_mix</a>(m_genes:u64, s_genes: u64):u64{
    <b>let</b> _new_genes:u64 = <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(m_genes)*<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(s_genes);
    _new_genes = <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(m_genes)*<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(s_genes);
    _new_genes = <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(m_genes)*<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(s_genes);
    _new_genes = <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(m_genes)*<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(s_genes);
    _new_genes = <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(m_genes)*<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(s_genes);
    _new_genes = <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(m_genes)*<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(s_genes);
    <a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(m_genes)*<a href="xtl_kitty.md#0x1_xtl_kitty_breeding_sqrt">sqrt</a>(s_genes)
}
</code></pre>



</details>


[move-book]: https://aptos.dev/move/book/SUMMARY
