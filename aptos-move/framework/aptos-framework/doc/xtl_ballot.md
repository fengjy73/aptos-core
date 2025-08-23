
<a id="0x1_xtl_ballot"></a>

# Module `0x1::xtl_ballot`



-  [Struct `Proposal`](#0x1_xtl_ballot_Proposal)
-  [Resource `Proposals`](#0x1_xtl_ballot_Proposals)
-  [Constants](#@Constants_0)
-  [Function `initialize`](#0x1_xtl_ballot_initialize)
-  [Function `vote`](#0x1_xtl_ballot_vote)
-  [Function `finalize`](#0x1_xtl_ballot_finalize)


<pre><code><b>use</b> <a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">0x1::signer</a>;
<b>use</b> <a href="../../aptos-stdlib/../move-stdlib/doc/string.md#0x1_string">0x1::string</a>;
</code></pre>



<a id="0x1_xtl_ballot_Proposal"></a>

## Struct `Proposal`



<pre><code><b>struct</b> <a href="xtl_ballot.md#0x1_xtl_ballot_Proposal">Proposal</a> <b>has</b> <b>copy</b>, drop, store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>name: <a href="../../aptos-stdlib/../move-stdlib/doc/string.md#0x1_string_String">string::String</a></code>
</dt>
<dd>

</dd>
<dt>
<code>totalVotedWeight: u64</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a id="0x1_xtl_ballot_Proposals"></a>

## Resource `Proposals`



<pre><code><b>struct</b> <a href="xtl_ballot.md#0x1_xtl_ballot_Proposals">Proposals</a> <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>proposals: <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector">vector</a>&lt;<a href="xtl_ballot.md#0x1_xtl_ballot_Proposal">xtl_ballot::Proposal</a>&gt;</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a id="@Constants_0"></a>

## Constants


<a id="0x1_xtl_ballot_FORUM_IS_NOT_EXISTED"></a>



<pre><code><b>const</b> <a href="xtl_ballot.md#0x1_xtl_ballot_FORUM_IS_NOT_EXISTED">FORUM_IS_NOT_EXISTED</a>: u64 = 1;
</code></pre>



<a id="0x1_xtl_ballot_initialize"></a>

## Function `initialize`



<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_ballot.md#0x1_xtl_ballot_initialize">initialize</a>(forum: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>, names: <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector">vector</a>&lt;<a href="../../aptos-stdlib/../move-stdlib/doc/string.md#0x1_string_String">string::String</a>&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_ballot.md#0x1_xtl_ballot_initialize">initialize</a>(forum: &<a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer">signer</a>, names: <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector">vector</a>&lt;String&gt;) <b>acquires</b> <a href="xtl_ballot.md#0x1_xtl_ballot_Proposals">Proposals</a>{

    <b>move_to</b>&lt;<a href="xtl_ballot.md#0x1_xtl_ballot_Proposals">Proposals</a>&gt;(forum, <a href="xtl_ballot.md#0x1_xtl_ballot_Proposals">Proposals</a>{
        proposals: <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_empty">vector::empty</a>&lt;<a href="xtl_ballot.md#0x1_xtl_ballot_Proposal">Proposal</a>&gt;()
    });

    <b>let</b> addr = <a href="../../aptos-stdlib/../move-stdlib/doc/signer.md#0x1_signer_address_of">signer::address_of</a>(forum);
    <b>let</b> proposal_collections = <b>borrow_global_mut</b>&lt;<a href="xtl_ballot.md#0x1_xtl_ballot_Proposals">Proposals</a>&gt;(addr);
    <b>let</b> len = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_length">vector::length</a>(&names);

    for( i in 0..len ){
        <b>let</b> proposal_name = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow">vector::borrow</a>&lt;String&gt;(&names,i);
        <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_push_back">vector::push_back</a>(&<b>mut</b> proposal_collections.proposals, <a href="xtl_ballot.md#0x1_xtl_ballot_Proposal">Proposal</a>{
            name: *proposal_name,
            totalVotedWeight: 0,
        });
    }
}
</code></pre>



</details>

<a id="0x1_xtl_ballot_vote"></a>

## Function `vote`



<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_ballot.md#0x1_xtl_ballot_vote">vote</a>(forum: <b>address</b>, proposal_index: u64)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="xtl_ballot.md#0x1_xtl_ballot_vote">vote</a>(forum: <b>address</b>, proposal_index: u64) <b>acquires</b> <a href="xtl_ballot.md#0x1_xtl_ballot_Proposals">Proposals</a> {
    <b>assert</b>!(<b>exists</b>&lt;<a href="xtl_ballot.md#0x1_xtl_ballot_Proposals">Proposals</a>&gt;(forum), <a href="xtl_ballot.md#0x1_xtl_ballot_FORUM_IS_NOT_EXISTED">FORUM_IS_NOT_EXISTED</a>);
    <b>let</b> proposal_collections = &<b>mut</b> <b>borrow_global_mut</b>&lt;<a href="xtl_ballot.md#0x1_xtl_ballot_Proposals">Proposals</a>&gt;(forum).proposals;
    <b>let</b> p = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow_mut">vector::borrow_mut</a>&lt;<a href="xtl_ballot.md#0x1_xtl_ballot_Proposal">Proposal</a>&gt;(proposal_collections,proposal_index);
    p.totalVotedWeight = p.totalVotedWeight + 1;
}
</code></pre>



</details>

<a id="0x1_xtl_ballot_finalize"></a>

## Function `finalize`



<pre><code>#[view]
<b>public</b> <b>fun</b> <a href="xtl_ballot.md#0x1_xtl_ballot_finalize">finalize</a>(forums: <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector">vector</a>&lt;<b>address</b>&gt;): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="xtl_ballot.md#0x1_xtl_ballot_finalize">finalize</a>(forums: <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector">vector</a>&lt;<b>address</b>&gt;): u64 <b>acquires</b> <a href="xtl_ballot.md#0x1_xtl_ballot_Proposals">Proposals</a>{
    <b>let</b> forum_nums = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_length">vector::length</a>(&forums);
    <b>let</b> res = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_empty">vector::empty</a>&lt;u64&gt;();
    {
        <b>let</b> tmp_forum = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow">vector::borrow</a>&lt;<b>address</b>&gt;(&forums,0);
        <b>let</b> proposals = <b>borrow_global</b>&lt;<a href="xtl_ballot.md#0x1_xtl_ballot_Proposals">Proposals</a>&gt;(*tmp_forum).proposals;
        <b>let</b> proposals_num = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_length">vector::length</a>(&proposals);
        for(i in 0..proposals_num){
            <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_push_back">vector::push_back</a>(&<b>mut</b> res,0);
        }
    };
    for(i in 0..forum_nums){
        <b>let</b> forum = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow">vector::borrow</a>&lt;<b>address</b>&gt;(&forums,i);
        <b>let</b> proposals = <b>borrow_global</b>&lt;<a href="xtl_ballot.md#0x1_xtl_ballot_Proposals">Proposals</a>&gt;(*forum).proposals;
        <b>let</b> proposals_num = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_length">vector::length</a>(&proposals);
        for( j in 0..proposals_num){
            <b>let</b> v = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow_mut">vector::borrow_mut</a>&lt;u64&gt;(&<b>mut</b> res,j);
            <b>let</b> w = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow">vector::borrow</a>&lt;<a href="xtl_ballot.md#0x1_xtl_ballot_Proposal">Proposal</a>&gt;(&proposals,j);
            *v = w.totalVotedWeight + *v;
        }
    };
    <b>let</b> winer = 0;
    <b>let</b> max_weight = 0;
    <b>let</b> len = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_length">vector::length</a>(&res);
    for( i in 0..len){
        <b>let</b> v = <a href="../../aptos-stdlib/../move-stdlib/doc/vector.md#0x1_vector_borrow">vector::borrow</a>&lt;u64&gt;(&res,i);
        <b>if</b>(max_weight &gt; *v){
            winer = i;
            max_weight = *v;
        }
    };
    winer
}
</code></pre>



</details>


[move-book]: https://aptos.dev/move/book/SUMMARY
