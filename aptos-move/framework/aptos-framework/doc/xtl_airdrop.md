
<a id="0x1_xtl_airdrop"></a>

# Module `0x1::xtl_airdrop`



-  [Struct `Coin`](#0x1_xtl_airdrop_Coin)
-  [Resource `CoinStore`](#0x1_xtl_airdrop_CoinStore)
-  [Constants](#@Constants_0)
-  [Function `initialize`](#0x1_xtl_airdrop_initialize)
-  [Function `transfer_n`](#0x1_xtl_airdrop_transfer_n)
-  [Function `getBalance`](#0x1_xtl_airdrop_getBalance)
-  [Function `deposit`](#0x1_xtl_airdrop_deposit)
-  [Function `withdraw`](#0x1_xtl_airdrop_withdraw)


<pre><code><b>use</b> <a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">0x1::signer</a>;
</code></pre>



<a id="0x1_xtl_airdrop_Coin"></a>

## Struct `Coin`



<pre><code><b>struct</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_Coin">Coin</a> <b>has</b> store
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

<a id="0x1_xtl_airdrop_CoinStore"></a>

## Resource `CoinStore`



<pre><code><b>struct</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_CoinStore">CoinStore</a> <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code><a href="coin.md#0x1_coin">coin</a>: <a href="xtl_airdrop.md#0x1_xtl_airdrop_Coin">xtl_airdrop::Coin</a></code>
</dt>
<dd>

</dd>
</dl>


</details>

<a id="@Constants_0"></a>

## Constants


<a id="0x1_xtl_airdrop_AIRDROP_AMOUNT"></a>



<pre><code><b>const</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_AIRDROP_AMOUNT">AIRDROP_AMOUNT</a>: u64 = 100;
</code></pre>



<a id="0x1_xtl_airdrop_INSUFFICIENT_BALANCE"></a>



<pre><code><b>const</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_INSUFFICIENT_BALANCE">INSUFFICIENT_BALANCE</a>: u64 = 3;
</code></pre>



<a id="0x1_xtl_airdrop_THE_ACCOUNT_IS_NOT_EXISTED"></a>



<pre><code><b>const</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_THE_ACCOUNT_IS_NOT_EXISTED">THE_ACCOUNT_IS_NOT_EXISTED</a>: u64 = 2;
</code></pre>



<a id="0x1_xtl_airdrop_initialize"></a>

## Function `initialize`



<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_initialize">initialize</a>(addr: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>, amount: u64)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_initialize">initialize</a>(addr: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a> ,amount : u64) {
    <b>move_to</b>&lt;<a href="xtl_airdrop.md#0x1_xtl_airdrop_CoinStore">CoinStore</a>&gt;(addr, <a href="xtl_airdrop.md#0x1_xtl_airdrop_CoinStore">CoinStore</a>{<a href="coin.md#0x1_coin">coin</a>: <a href="xtl_airdrop.md#0x1_xtl_airdrop_Coin">Coin</a>{value:amount}});
}
</code></pre>



</details>

<a id="0x1_xtl_airdrop_transfer_n"></a>

## Function `transfer_n`



<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_transfer_n">transfer_n</a>(from: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>, recipients: <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector">vector</a>&lt;<b>address</b>&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_transfer_n">transfer_n</a>(from: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>,recipients: <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector">vector</a>&lt;<b>address</b>&gt;) <b>acquires</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_CoinStore">CoinStore</a> {

    <b>let</b> len = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_length">vector::length</a>(&recipients);
    <b>let</b> from_addr = <a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer_address_of">signer::address_of</a>(from);

    <b>let</b> balance = <a href="xtl_airdrop.md#0x1_xtl_airdrop_getBalance">getBalance</a>(from_addr);

    <b>if</b>(balance &gt;= <a href="xtl_airdrop.md#0x1_xtl_airdrop_AIRDROP_AMOUNT">AIRDROP_AMOUNT</a> * len){
        for(i in 0..len){
            <b>let</b> <b>to</b> = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow">vector::borrow</a>&lt;<b>address</b>&gt;(&recipients,i);
            <b>let</b> <a href="coin.md#0x1_coin">coin</a> = <a href="xtl_airdrop.md#0x1_xtl_airdrop_withdraw">withdraw</a>(from_addr, <a href="xtl_airdrop.md#0x1_xtl_airdrop_AIRDROP_AMOUNT">AIRDROP_AMOUNT</a>);
            <a href="xtl_airdrop.md#0x1_xtl_airdrop_deposit">deposit</a>(*<b>to</b>, <a href="coin.md#0x1_coin">coin</a>);
        };
    }

}
</code></pre>



</details>

<a id="0x1_xtl_airdrop_getBalance"></a>

## Function `getBalance`



<pre><code><b>public</b> <b>fun</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_getBalance">getBalance</a>(owner: <b>address</b>): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_getBalance">getBalance</a>(owner: <b>address</b>) : u64 <b>acquires</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_CoinStore">CoinStore</a>{
    <b>assert</b>!(<b>exists</b>&lt;<a href="xtl_airdrop.md#0x1_xtl_airdrop_CoinStore">CoinStore</a>&gt;(owner), <a href="xtl_airdrop.md#0x1_xtl_airdrop_THE_ACCOUNT_IS_NOT_EXISTED">THE_ACCOUNT_IS_NOT_EXISTED</a>);
    <b>borrow_global</b>&lt;<a href="xtl_airdrop.md#0x1_xtl_airdrop_CoinStore">CoinStore</a>&gt;(owner).<a href="coin.md#0x1_coin">coin</a>.value
}
</code></pre>



</details>

<a id="0x1_xtl_airdrop_deposit"></a>

## Function `deposit`



<pre><code><b>fun</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_deposit">deposit</a>(account_addr: <b>address</b>, <a href="coin.md#0x1_coin">coin</a>: <a href="xtl_airdrop.md#0x1_xtl_airdrop_Coin">xtl_airdrop::Coin</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_deposit">deposit</a>(account_addr : <b>address</b>, <a href="coin.md#0x1_coin">coin</a> : <a href="xtl_airdrop.md#0x1_xtl_airdrop_Coin">Coin</a>) <b>acquires</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_CoinStore">CoinStore</a> {
    <b>assert</b>!(<b>exists</b>&lt;<a href="xtl_airdrop.md#0x1_xtl_airdrop_CoinStore">CoinStore</a>&gt;(account_addr), <a href="xtl_airdrop.md#0x1_xtl_airdrop_THE_ACCOUNT_IS_NOT_EXISTED">THE_ACCOUNT_IS_NOT_EXISTED</a>);
    <b>let</b> balance = <a href="xtl_airdrop.md#0x1_xtl_airdrop_getBalance">getBalance</a>(account_addr);
    <b>let</b> balance_ref = &<b>mut</b> <b>borrow_global_mut</b>&lt;<a href="xtl_airdrop.md#0x1_xtl_airdrop_CoinStore">CoinStore</a>&gt;(account_addr).<a href="coin.md#0x1_coin">coin</a>.value;
    *balance_ref = balance + <a href="coin.md#0x1_coin">coin</a>.value;
    <b>let</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_Coin">Coin</a> { value:_ } = <a href="coin.md#0x1_coin">coin</a>;
}
</code></pre>



</details>

<a id="0x1_xtl_airdrop_withdraw"></a>

## Function `withdraw`



<pre><code><b>fun</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_withdraw">withdraw</a>(account_addr: <b>address</b>, amount: u64): <a href="xtl_airdrop.md#0x1_xtl_airdrop_Coin">xtl_airdrop::Coin</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_withdraw">withdraw</a>(account_addr : <b>address</b>, amount : u64) : <a href="xtl_airdrop.md#0x1_xtl_airdrop_Coin">Coin</a> <b>acquires</b> <a href="xtl_airdrop.md#0x1_xtl_airdrop_CoinStore">CoinStore</a> {
    <b>assert</b>!(<b>exists</b>&lt;<a href="xtl_airdrop.md#0x1_xtl_airdrop_CoinStore">CoinStore</a>&gt;(account_addr), <a href="xtl_airdrop.md#0x1_xtl_airdrop_THE_ACCOUNT_IS_NOT_EXISTED">THE_ACCOUNT_IS_NOT_EXISTED</a>);
    <b>let</b> balance = <a href="xtl_airdrop.md#0x1_xtl_airdrop_getBalance">getBalance</a>(account_addr);
    <b>assert</b>!(balance &gt;= amount, <a href="xtl_airdrop.md#0x1_xtl_airdrop_INSUFFICIENT_BALANCE">INSUFFICIENT_BALANCE</a>);
    <b>let</b> balance_ref = &<b>mut</b> <b>borrow_global_mut</b>&lt;<a href="xtl_airdrop.md#0x1_xtl_airdrop_CoinStore">CoinStore</a>&gt;(account_addr).<a href="coin.md#0x1_coin">coin</a>.value;
    *balance_ref = balance - amount;
    <a href="xtl_airdrop.md#0x1_xtl_airdrop_Coin">Coin</a> { value: amount }
}
</code></pre>



</details>


[move-book]: https://aptos.dev/move/book/SUMMARY
