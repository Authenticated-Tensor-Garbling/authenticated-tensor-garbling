// circuit to garble: AES-128, pull it in --> no, we want to just do some random inputs and outputs.

// set the csp to 128 and ssp to 40.
const CSP: usize = 128;
const SSP: usize = 40;

// set the key and the message to random inputs

// write insecure Fpre
// define sigma = 2ssp
// run Fpre(1^ssp, 1^csp, Circuit, sigma) -> pre_a, pre_b
    // delta_a
    // delta_b
    // Authbits for every wire in the circuit
    // and correlated authbits for every AND gate output wire s* = (in0_a xor in0_b) ^ (in1_a xor in1_b)

// write functions Garble_G, Garble_E, Eval which match our paper's KRRW
// these will be isolated functions with interfaces that match the paper

// Garble_G(1^csp, circuit, pre_a) -> garbledcircuit, array of garbled gates
    // for each input wire, define zero label and the one label = zero XOR delta_a
    // for every XOR gate, define the output zero = input zeros XORed, and output as above
    // for every AND gate,
        // compute the table in out paper
    // output all wire labels and the GC share

// will need to do input processing, or just deliver random labels (no need for OT)

// Garble_E(1^csp, circuit, pre_b)
    // AND gates only:
        // for each alpha, beta, gamma, AND
        // compute the table in the paper (just MACs really)
    // output the garbled material 

// Eval(1^csp, circuit (what is this), GC_a, GC_b, all masked labels for input wires)
    // following the circuits topology (look at the and gate / switch statement in garble-core)
    // if XOR gate
        // just XOR the labels given
    // if AND gate
        // put the half gate shares together
        // evaluate the half gates
        // reconstruct the output label
        // extract the masked value from the label
    // output the output labels (or in our case, every label on every wire)

// compose all of the above and check correctness

pub(crate) async fn garble_gen(
    ctx: &mut Context,                  // ThreadID, Io, Mode
    circ: Arc<Circuit>,                 // Circuit
    delta: Delta,                       // Delta
    input_labels: &[Key],               // why is this a Key? look at KRRW
    input_auth_bits: &[AuthBitShare],   // share of authbits
    shares: &[AuthBitShare],            // intermediate auth bits? look at implementation
) -> Result<AuthGenOutput, AuthGeneratorError> {
    // Use cointoss to agree on a random seed
    let sender_seed = vec![Block::random(&mut rand::rng())];
    let sender_output = cointoss_sender(ctx, sender_seed).await?;
    let seed = u64::from_le_bytes(sender_output[0].as_bytes()[..8].try_into().unwrap());
    
    let bucket_size = (SSP as f64 / (circ.and_count() as f64).log2()).ceil() as usize;
    let mut gb = AuthGenCore::new(seed, bucket_size);
    let io = ctx.io_mut();

    // Function independent pre-processing: using auth bits to generate auth triples
    let (c, mut g) = gb.generate_pre_1(&circ, delta, input_auth_bits, shares).unwrap();
    io.feed(g.clone()).await?;
    io.flush().await?;
    let gr: Vec<Block>  = io.expect_next().await?;

    let d = gb.generate_pre_2(delta, c, &mut g, gr).unwrap();
    io.feed(d.clone()).await?;
    io.flush().await?;
    let dr: Vec<bool> = io.expect_next().await?;

    let data = gb.generate_pre_3(delta, &mut g, d, dr).unwrap();
    
    
    // Secure equality check for authenticity of triples
    let (digest, salt, hash) = gb.check_equality(g).unwrap();
    io.feed(hash).await?;
    io.flush().await?;
    
    let digest_recv: Block = io.expect_next().await?;
    if digest != digest_recv {
        return Err(AuthGeneratorError(ErrorRepr::EqualityCheckFailed));
    }

    io.feed(salt).await?;
    io.flush().await?;

    
    io.feed(data.clone()).await?;
    io.flush().await?;
    let data_recv: Vec<bool> = io.expect_next().await?;

    gb.generate_pre_4(data, data_recv).unwrap();
    
    // Function dependent pre-processing: generate auth bits for all wires in the circuit
    gb.generate_free(&circ).unwrap();
    let (px, py) = gb.generate_de(&circ).unwrap();
    io.feed((px,py)).await?;
    io.flush().await?;
    let (px_recv, py_recv): (Vec<bool>, Vec<bool>) = io.expect_next().await?;

    let mut gb_iter= gb.generate_batched(&circ, delta, &input_labels, px_recv, py_recv).unwrap();
    while let Some(batch) = gb_iter.by_ref().next() {
        io.feed(batch).await?;
    }
    io.flush().await?;

    // TODO: For preprocessing, receive all the masked values at the end
    let masked_values: Vec<bool> = io.expect_next().await?;
    Ok(gb_iter.finish(masked_values)?)
}

pub(crate) async fn garble_eval(
    ctx: &mut Context,                  // ThreadID, Io, Mode
    circ: Arc<Circuit>,                 // Circuit
    delta: Delta,                       // Delta
    input_labels: &[Mac],               // why is this a MAC? look at KRRW
    masked_inputs: Vec<bool>,           // bitstring of masked booleans
    input_auth_bits: &[AuthBitShare],   // share of authbits
    shares: &[AuthBitShare],            // intermediate auth bits? look at implementation
) -> Result<AuthEvalOutput, AuthEvaluatorError> {
    // Use cointoss to agree on a random seed
    let receiver_seed = vec![Block::random(&mut rand::rng())];
    let receiver_output = cointoss_receiver(ctx, receiver_seed).await?;
    let seed = u64::from_le_bytes(receiver_output[0].as_bytes()[..8].try_into().unwrap());
    
    let bucket_size = (SSP as f64 / (circ.and_count() as f64).log2()).ceil() as usize;
    let mut ev = AuthEvalCore::new(seed, bucket_size);
    let io = ctx.io_mut();  

    // Function independent pre-processing: using auth bits to generate auth triples
    let (c, mut g) = ev.evaluate_pre_1(&circ, delta, input_auth_bits, shares).unwrap();
    io.feed(g.clone()).await?;
    io.flush().await?;
    let gr: Vec<Block>  = io.expect_next().await?;

    let d = ev.evaluate_pre_2(delta, c, &mut g, gr).unwrap();
    io.feed(d.clone()).await?;
    io.flush().await?;
    let dr: Vec<bool> = io.expect_next().await?;

    let data = ev.evaluate_pre_3(delta, &mut g, d, dr).unwrap();
    

    // Secure equality check for authenticity of triples
    let digest = ev.check_equality(g).unwrap();
    let hash_recv: Block = io.expect_next().await?;
    io.feed(digest).await?;
    io.flush().await?;
    
    let salt_recv: Block = io.expect_next().await?;

    let expected_hash = ev.check_salt(salt_recv, digest).unwrap();
    if expected_hash != hash_recv {
        return Err(AuthEvaluatorError(ErrorRepr::EqualityCheckFailed));
    }

    io.feed(data.clone()).await?;
    io.flush().await?;
    let data_recv: Vec<bool> = io.expect_next().await?;

    ev.evaluate_pre_4(data, data_recv).unwrap();
    
    
    // Function dependent pre-processing: generate auth bits for all wires in the circuit
    ev.evaluate_free(&circ).unwrap();
    let (px, py) = ev.evaluate_de(&circ).unwrap();
    io.feed((px,py)).await?;
    io.flush().await?;
    let (px_recv, py_recv): (Vec<bool>, Vec<bool>) = io.expect_next().await?;

    let mut ev_consumer = ev.evaluate_batched(&circ, delta, &input_labels, masked_inputs, px_recv, py_recv).unwrap();

    while ev_consumer.wants_gates() {
        let batch: AuthHalfGateBatch = io.expect_next().await?;
        ev_consumer.next(batch);
    }

    let output = ev_consumer.finish()?;
    let masked_values = output.masked_values.clone();

    // TODO: For preprocessing, send all the masked values at the end
    io.feed(masked_values).await?;
    io.flush().await?;

    Ok(output)
}