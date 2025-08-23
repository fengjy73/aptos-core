
<a id="0x1_xtl_million_pixel"></a>

# Module `0x1::xtl_million_pixel`



-  [Struct `Land`](#0x1_xtl_million_pixel_Land)
-  [Resource `LandStore`](#0x1_xtl_million_pixel_LandStore)
-  [Function `initialize`](#0x1_xtl_million_pixel_initialize)
-  [Function `occupy`](#0x1_xtl_million_pixel_occupy)


<pre><code><b>use</b> <a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">0x1::signer</a>;
<b>use</b> <a href="../../aptos-stdlib/doc/table.md#0x1_table">0x1::table</a>;
</code></pre>



<a id="0x1_xtl_million_pixel_Land"></a>

## Struct `Land`



<pre><code><b>struct</b> <a href="xtl_million_pixel.md#0x1_xtl_million_pixel_Land">Land</a> <b>has</b> drop, store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>flag: bool</code>
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

<a id="0x1_xtl_million_pixel_LandStore"></a>

## Resource `LandStore`



<pre><code><b>struct</b> <a href="xtl_million_pixel.md#0x1_xtl_million_pixel_LandStore">LandStore</a> <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>land: <a href="../../aptos-stdlib/doc/table.md#0x1_table_Table">table::Table</a>&lt;u32, <a href="xtl_million_pixel.md#0x1_xtl_million_pixel_Land">xtl_million_pixel::Land</a>&gt;</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a id="0x1_xtl_million_pixel_initialize"></a>

## Function `initialize`



<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_million_pixel.md#0x1_xtl_million_pixel_initialize">initialize</a>(global_store: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_million_pixel.md#0x1_xtl_million_pixel_initialize">initialize</a>(global_store: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>){
    <b>move_to</b>(global_store,<a href="xtl_million_pixel.md#0x1_xtl_million_pixel_LandStore">LandStore</a>{
        land: <a href="../../aptos-stdlib/doc/table.md#0x1_table_new">table::new</a>()
    });
}
</code></pre>



</details>

<a id="0x1_xtl_million_pixel_occupy"></a>

## Function `occupy`



<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_million_pixel.md#0x1_xtl_million_pixel_occupy">occupy</a>(sender: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>, global_land: <b>address</b>, x: u16, y: u16)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_million_pixel.md#0x1_xtl_million_pixel_occupy">occupy</a>(sender: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>,global_land: <b>address</b>,x:u16, y:u16) <b>acquires</b> <a href="xtl_million_pixel.md#0x1_xtl_million_pixel_LandStore">LandStore</a>{
    <b>let</b> addr = <a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer_address_of">signer::address_of</a>(sender);
    <b>let</b> index:u32 = (x <b>as</b> u32) * 65536 + (y <b>as</b> u32);
    <b>let</b> land_mp = <b>borrow_global_mut</b>&lt;<a href="xtl_million_pixel.md#0x1_xtl_million_pixel_LandStore">LandStore</a>&gt;(global_land);
    <b>if</b>(<a href="../../aptos-stdlib/doc/table.md#0x1_table_contains">table::contains</a>(&<b>mut</b> land_mp.land, index) == <b>false</b>){
        <a href="../../aptos-stdlib/doc/table.md#0x1_table_add">table::add</a>(&<b>mut</b> land_mp.land, index, <a href="xtl_million_pixel.md#0x1_xtl_million_pixel_Land">Land</a>{
            flag:<b>true</b>,
            owner:addr
        });
    };
}
</code></pre>



</details>


[move-book]: https://aptos.dev/move/book/SUMMARY
